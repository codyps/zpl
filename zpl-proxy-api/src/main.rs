mod listener;
mod png_metadata;
mod printer;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    error_handling::HandleError,
    extract::{Form, FromRequest, Json, Multipart, State},
    http::{header, HeaderName, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    routing::post,
    RequestExt, Router,
};
use clap::Parser;
use fastrace::{future::FutureExt, local::LocalSpan, Span};
use serde::Deserialize;
use tower_layer::Layer;
use zpl_proxy_api::cache::{self, Cache};
use zpl_proxy_api::telemetry;
use zpl_proxy_api::validation::{RenderZpl, ValidationError};

#[derive(Debug, Parser)]
struct Args {
    #[clap(long)]
    zd621_url: reqwest::Url,
    /// SGD IP:port for the same printer; defaults to the HTTP URL host on 9100.
    #[clap(long)]
    zd621_sgd_address: Option<std::net::SocketAddr>,
    #[clap(long)]
    zd621_header: Vec<String>,
    #[command(flatten)]
    listen: listener::ListenOptions,
    /// Change after printer firmware, fonts, media, or other rendering state changes.
    #[clap(long, default_value = "default")]
    cache_namespace: String,
}

#[derive(Clone)]
struct Zd621 {
    zd621_client: reqwest::Client,
    zd621_url: Arc<reqwest::Url>,
    sgd_target: Arc<String>,
    cache: Cache,
    renderer_key: Vec<u8>,
    render_lock: Arc<tokio::sync::Mutex<()>>,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    // Consume activation environment and bind before runtime/exporter threads.
    let listener = listener::Listener::open(&args.listen)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(args, listener))
}

async fn run(args: Args, listener: listener::Listener) -> eyre::Result<()> {
    let _telemetry = telemetry::init().expect("initialize telemetry");

    let zd621_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        // The printer rejects lowercase HTTP/1 header names.
        .http1_title_case_headers()
        .default_headers(
            args.zd621_header
                .iter()
                .map(|header| {
                    let (key, value) = header.split_once(": ").unwrap();
                    let key: HeaderName = key.parse().unwrap();
                    let value: HeaderValue = value.parse().unwrap();
                    (key, value)
                })
                .collect(),
        )
        .build()
        .unwrap();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let cache = tokio::task::spawn_blocking(move || Cache::open(&database_url))
        .await
        .unwrap()
        .expect("Cannot open cache database; run diesel migration run first");
    let sgd_target = printer::target(&args.zd621_url, args.zd621_sgd_address)?;
    let renderer_key = cache::renderer_key(
        args.zd621_url.as_str(),
        &args.zd621_header,
        &args.cache_namespace,
        &sgd_target,
    );
    let app_state = Zd621 {
        zd621_client,
        zd621_url: Arc::new(args.zd621_url),
        sgd_target: Arc::new(sgd_target),
        cache,
        renderer_key,
        render_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    let livereload = tower_livereload::LiveReloadLayer::new();

    let api_router = Router::new()
        .route("/zpl-zd621", post(zd621_zpl_to_png))
        .with_state(app_state);

    let app = Router::new()
        .nest("/api", api_router)
        // Axum 0.8 rejects nesting at "/"; use the static service as fallback.
        // https://docs.rs/axum/0.8.9/axum/struct.Router.html#method.fallback_service
        .fallback_service(HandleError::new(
            livereload.layer(tower_http::services::ServeDir::new(Path::new("assets"))),
            |error| async move {
                log::error!("static asset service error: {}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            },
        ))
        .layer(axum::middleware::from_fn(telemetry::request));

    listener.serve(app).await?;
    Ok(())
}

#[derive(Deserialize)]
struct PrintSpec {
    zpl: String,
    #[serde(default)]
    refresh: bool,
}

async fn zd621_zpl_to_png(
    State(zd621): State<Zd621>,
    JsonOrForm(print_spec): JsonOrForm,
) -> impl IntoResponse {
    // Finish persistence even when the requesting client disconnects.
    let result = telemetry::spawn("render.task", render_cached(zd621, print_spec)).await;
    let result = result.map(|result| {
        result.and_then(|(png, cache_hit, identity)| {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header("X-ZPL-Cache", if cache_hit { "hit" } else { "miss" })
                .header("X-ZPL-Printer-Model", &identity.model)
                .header("X-ZPL-Printer-Firmware", &identity.firmware)
                .header("X-ZPL-Printer-Serial", &identity.serial);
            Ok(response.body(Body::from(png))?)
        })
    });
    match result {
        Ok(Ok(response)) => response,
        Ok(Err(error)) if error.downcast_ref::<ValidationError>().is_some() => (
            StatusCode::BAD_REQUEST,
            error.downcast_ref::<ValidationError>().unwrap().to_string(),
        )
            .into_response(),
        _ => {
            log::error!("render or cache operation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to render and persist the label",
            )
                .into_response()
        }
    }
}

#[fastrace::trace(name = "render.cached")]
async fn render_cached(
    zd621: Zd621,
    print_spec: PrintSpec,
) -> eyre::Result<(Vec<u8>, bool, zebra_sgd::PrinterIdentity)> {
    // Validate before cache lookup too: historical cache entries are not proof
    // that the current admission policy permits this input.
    let zpl =
        telemetry::spawn_blocking("render.validate", move || RenderZpl::parse(print_spec.zpl))
            .await??;
    // Zebra uses a shared TEST1 preview object. Serialize the entire POST/GET
    // cycle as well as cache misses so concurrent requests cannot mix images.
    let _guard = zd621
        .render_lock
        .lock()
        .in_span(Span::enter_with_local_parent("printer.queue"))
        .await;
    let attempt = zd621
        .cache
        .begin(
            zpl.as_str().as_bytes().to_vec(),
            zd621.renderer_key.clone(),
            print_spec.refresh,
        )
        .await?;
    if let Some(png) = &attempt.cached_png {
        LocalSpan::add_property(|| ("cache.hit", "true"));
        let result = (|| {
            let identity = attempt
                .cached_identity
                .clone()
                .ok_or_else(|| eyre::eyre!("cached printer identity missing"))?;
            Ok::<_, eyre::Report>((
                png_metadata::annotate(png, &identity, zpl.as_str())?,
                true,
                identity,
            ))
        })();
        if let Err(error) = &result {
            zd621.cache.failure(attempt, error.to_string()).await?;
        }
        return result;
    }
    LocalSpan::add_property(|| ("cache.hit", "false"));
    let render = async {
        let identity = printer::identity((*zd621.sgd_target).clone()).await?;
        let png = zebra_http_api::zpl_to_png(
            zd621.zd621_client.clone(),
            (*zd621.zd621_url).clone(),
            zpl.as_str(),
        )
        .await?;
        Ok::<_, eyre::Report>((png, identity))
    };
    match render
        .in_span(
            Span::enter_with_local_parent("printer.preview")
                .with_property(|| ("span.kind", "client")),
        )
        .await
    {
        Ok((png, identity)) => {
            let response_png = png_metadata::annotate(&png, &identity, zpl.as_str());
            zd621
                .cache
                .rendered(
                    attempt,
                    png,
                    Some(identity.clone()),
                    response_png.as_ref().err().map(ToString::to_string),
                )
                .await?;
            Ok((response_png?, false, identity))
        }
        Err(e) => {
            LocalSpan::add_property(|| ("span.status_code", "error"));
            zd621.cache.failure(attempt, e.to_string()).await?;
            Err(e)
        }
    }
}

struct JsonOrForm(PrintSpec);

// Axum 0.8 extractors return native futures.
// https://docs.rs/axum/0.8/axum/extract/trait.FromRequest.html
impl<S> FromRequest<S> for JsonOrForm
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request(req: Request<Body>, _state: &S) -> Result<Self, Self::Rejection> {
        let content_type_header = req.headers().get(header::CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        if let Some(content_type) = content_type {
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }

            if content_type.starts_with("application/x-www-form-urlencoded") {
                let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }

            if content_type.starts_with("multipart/form-data") {
                // Axum enforces its default body limit for Multipart too.
                // https://docs.rs/axum/0.8/axum/extract/struct.Multipart.html
                let mut multipart: Multipart =
                    req.extract().await.map_err(IntoResponse::into_response)?;
                let mut zpl = None;
                let mut refresh = false;
                while let Some(field) = multipart
                    .next_field()
                    .await
                    .map_err(IntoResponse::into_response)?
                {
                    if field.name() == Some("zpl") {
                        // Preserve the old non-strict derive: unknown fields are
                        // ignored, and the last repeated zpl field wins.
                        let bytes = field.bytes().await.map_err(IntoResponse::into_response)?;
                        zpl = Some(String::from_utf8(bytes.to_vec()).map_err(|_| {
                            (StatusCode::BAD_REQUEST, "zpl field is not UTF-8").into_response()
                        })?);
                    } else if field.name() == Some("refresh") {
                        refresh = field
                            .text()
                            .await
                            .map_err(IntoResponse::into_response)?
                            .parse::<bool>()
                            .map_err(|_| {
                                (StatusCode::BAD_REQUEST, "refresh must be true or false")
                                    .into_response()
                            })?;
                    }
                }
                let zpl = zpl.ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, "missing zpl field").into_response()
                })?;
                return Ok(Self(PrintSpec { zpl, refresh }));
            }
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{connection::SimpleConnection, prelude::*};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn mock_identity(version: usize) -> zebra_sgd::PrinterIdentity {
        zebra_sgd::PrinterIdentity {
            model: "ZD621".into(),
            serial: "TEST-SERIAL".into(),
            firmware: format!("V93.21.{version}Z"),
            configuration: [
                ("zpl.label_length".into(), version.to_string()),
                ("head.resolution.in_dpi".into(), "203".into()),
                ("appl.link_os_version".into(), "7.0".into()),
                ("appl.bootblock".into(), "7.0.4 0.0".into()),
            ]
            .into(),
        }
    }

    async fn mock_sgd(
        version: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
        fail: Arc<AtomicBool>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                calls.fetch_add(1, Ordering::SeqCst);
                let identity = mock_identity(version.load(Ordering::SeqCst));
                let mut stream = BufReader::new(stream);
                loop {
                    let mut command = String::new();
                    match stream.read_line(&mut command).await {
                        Ok(0) => break,
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                        Err(error) => panic!("mock SGD read failed: {error}"),
                    }
                    let name = command
                        .strip_prefix("! U1 getvar \"")
                        .unwrap()
                        .strip_suffix("\"\r\n")
                        .unwrap();
                    let value = match name {
                        "device.product_name" => &identity.model,
                        "device.unique_id" if fail.load(Ordering::SeqCst) => "?",
                        "device.unique_id" => &identity.serial,
                        "appl.name" => &identity.firmware,
                        name => {
                            assert!(zebra_sgd::RENDER_SETTINGS.contains(&name));
                            identity
                                .configuration
                                .get(name)
                                .map(String::as_str)
                                .unwrap_or("?")
                        }
                    };
                    stream
                        .get_mut()
                        .write_all(format!("\"{value}\"\r\n").as_bytes())
                        .await
                        .unwrap();
                }
            }
        });
        (address, task)
    }

    #[tokio::test]
    async fn caches_mock_printer_results_and_persists_refresh_failures() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let mut connection = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        for migration in [
            include_str!("../migrations/2024-10-03-035443_cache-results/up.sql"),
            include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql"),
            include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
            include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
            include_str!("../migrations/2026-10-03-000000_printer-identity/up.sql"),
        ] {
            connection.batch_execute(migration).unwrap();
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let fail = Arc::new(AtomicBool::new(false));
        let firmware = Arc::new(AtomicUsize::new(33));
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let identity_fail = Arc::new(AtomicBool::new(false));
        let (sgd_target, sgd_server) = mock_sgd(
            firmware.clone(),
            identity_calls.clone(),
            identity_fail.clone(),
        )
        .await;
        let printer = Router::new()
            .route(
                "/zpl",
                post({
                    let calls = calls.clone();
                    let fail = fail.clone();
                    move || {
                        let calls = calls.clone();
                        let fail = fail.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            if fail.load(Ordering::SeqCst) {
                                (StatusCode::SERVICE_UNAVAILABLE, "offline")
                            } else {
                                (StatusCode::OK, "<IMG SRC=\"/image\">")
                            }
                        }
                    }
                }),
            )
            .route(
                "/image",
                axum::routing::get(|| async { png_metadata::test_png() }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url: reqwest::Url = format!("http://{}/", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, printer).await.unwrap();
        });
        let mut state = Zd621 {
            zd621_client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            renderer_key: cache::renderer_key(url.as_str(), &[], "test", &sgd_target),
            sgd_target: Arc::new(sgd_target),
            zd621_url: Arc::new(url),
            cache: Cache::open(path.to_str().unwrap()).unwrap(),
            render_lock: Arc::new(tokio::sync::Mutex::new(())),
        };
        let spec = || PrintSpec {
            zpl: "^XA^XZ".into(),
            refresh: false,
        };
        let (first, second) = tokio::join!(
            render_cached(state.clone(), spec()),
            render_cached(state.clone(), spec())
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(
            first.0,
            png_metadata::annotate(&png_metadata::test_png(), &first.2, &spec().zpl).unwrap()
        );
        assert_eq!(second.0, first.0);
        assert_ne!(first.1, second.1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert_eq!(identity_calls.load(Ordering::SeqCst), 1);
        // A cache hit after a firmware change must describe the original render.
        firmware.store(34, Ordering::SeqCst);
        state.cache = Cache::open(path.to_str().unwrap()).unwrap();
        let response = zd621_zpl_to_png(State(state.clone()), JsonOrForm(spec()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["X-ZPL-Cache"], "hit");
        assert_eq!(response.headers()["X-ZPL-Printer-Model"], "ZD621");
        assert_eq!(response.headers()["X-ZPL-Printer-Firmware"], "V93.21.33Z");
        assert_eq!(response.headers()["X-ZPL-Printer-Serial"], "TEST-SERIAL");
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap()
                .as_ref(),
            first.0
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(identity_calls.load(Ordering::SeqCst), 1);
        fail.store(true, Ordering::SeqCst);
        assert!(render_cached(
            state.clone(),
            PrintSpec {
                refresh: true,
                ..spec()
            }
        )
        .await
        .is_err());
        fail.store(false, Ordering::SeqCst);
        let refreshed = render_cached(state.clone(), spec()).await.unwrap();
        assert!(!refreshed.1);
        assert_eq!(refreshed.2.firmware, "V93.21.34Z");
        assert_eq!(refreshed.2.configuration["zpl.label_length"], "34");
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        use zpl_proxy_api::{
            models::PngRequest,
            schema::{inputs, png_requests, pngs},
        };
        let history = png_requests::table
            .order(png_requests::rowid)
            .select(PngRequest::as_select())
            .load::<PngRequest>(&mut connection)
            .unwrap();
        assert_eq!(history.len(), 5);
        assert!(history.iter().all(|r| r.completed_at.is_some()));
        assert!(history[3].error.as_ref().unwrap().contains("503"));
        for request in &history[..3] {
            let identity: zebra_sgd::PrinterIdentity =
                serde_json::from_str(request.printer_identity.as_ref().unwrap()).unwrap();
            assert_eq!(identity.firmware, "V93.21.33Z");
            assert_eq!(identity.configuration["zpl.label_length"], "33");
        }
        let latest: zebra_sgd::PrinterIdentity =
            serde_json::from_str(history[4].printer_identity.as_ref().unwrap()).unwrap();
        assert_eq!(latest.firmware, "V93.21.34Z");
        // Original bytes and their digest survive annotation and firmware changes.
        let (stored, hash) = pngs::table
            .select((pngs::data, pngs::hash))
            .first::<(Vec<u8>, Vec<u8>)>(&mut connection)
            .unwrap();
        assert_eq!(stored, png_metadata::test_png());
        use sha2::Digest;
        assert_eq!(hash, sha2::Sha256::digest(&stored).to_vec());
        assert_eq!(
            inputs::table
                .count()
                .get_result::<i64>(&mut connection)
                .unwrap(),
            1
        );
        assert_eq!(
            pngs::table
                .count()
                .get_result::<i64>(&mut connection)
                .unwrap(),
            1
        );
        // Failed metadata discovery must be recorded, invalidate refresh, and
        // never submit a preview with an unknown printer identity.
        identity_fail.store(true, Ordering::SeqCst);
        assert!(render_cached(
            state.clone(),
            PrintSpec {
                refresh: true,
                ..spec()
            }
        )
        .await
        .is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let error = png_requests::table
            .order(png_requests::rowid.desc())
            .select(png_requests::error)
            .first::<Option<String>>(&mut connection)
            .unwrap()
            .unwrap();
        assert!(error.contains("unsupported SGD variable device.unique_id"));
        identity_fail.store(false, Ordering::SeqCst);
        assert!(!render_cached(state, spec()).await.unwrap().1);
        server.abort();
        sgd_server.abort();
    }

    #[tokio::test]
    async fn rejects_unsafe_zpl_in_every_http_format_before_cache_or_printer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.sqlite");
        let mut connection = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        for migration in [
            include_str!("../migrations/2024-10-03-035443_cache-results/up.sql"),
            include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql"),
            include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
            include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
            include_str!("../migrations/2026-10-03-000000_printer-identity/up.sql"),
        ] {
            connection.batch_execute(migration).unwrap();
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let (sgd_target, sgd_server) = mock_sgd(
            Arc::new(AtomicUsize::new(33)),
            identity_calls.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
        let printer = Router::new()
            .route(
                "/zpl",
                post({
                    let calls = calls.clone();
                    move || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        async { "<IMG SRC=\"/image\">" }
                    }
                }),
            )
            .route(
                "/image",
                axum::routing::get(|| async { png_metadata::test_png() }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url: reqwest::Url = format!("http://{}/", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let printer_task =
            tokio::spawn(async move { axum::serve(listener, printer).await.unwrap() });
        let state = Zd621 {
            zd621_client: reqwest::Client::builder().no_proxy().build().unwrap(),
            renderer_key: cache::renderer_key(url.as_str(), &[], "test", &sgd_target),
            sgd_target: Arc::new(sgd_target),
            zd621_url: Arc::new(url),
            cache: Cache::open(path.to_str().unwrap()).unwrap(),
            render_lock: Arc::new(tokio::sync::Mutex::new(())),
        };
        // Simulate a result cached before admission validation existed. The new
        // policy must not disclose it, including when refresh is requested.
        let forbidden = "^XA^XGR:PRIVATE.GRF,1,1^FS^XZ";
        let attempt = state
            .cache
            .begin(
                forbidden.as_bytes().to_vec(),
                state.renderer_key.clone(),
                false,
            )
            .await
            .unwrap();
        state
            .cache
            .success(attempt, b"private cached PNG".to_vec(), None)
            .await
            .unwrap();
        let app = Router::new()
            .route("/api/zpl-zd621", post(zd621_zpl_to_png))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/api/zpl-zd621", listener.local_addr().unwrap());
        let proxy_task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for zpl in [
            forbidden,
            "^XA^FO1,2^FDhello^FS^XZ^",
            "^XA^WD*:*.*^XZ",
            "^XA^FXcomment\n! U1 getvar \"allcv\"\n^FS^XZ",
        ] {
            for refresh in [false, true] {
                let requests = [
                    client
                        .post(&endpoint)
                        .json(&serde_json::json!({ "zpl": zpl, "refresh": refresh })),
                    client.post(&endpoint).form(&[
                        ("zpl", zpl),
                        ("refresh", if refresh { "true" } else { "false" }),
                    ]),
                    client
                        .post(&endpoint)
                        .header(header::CONTENT_TYPE, MULTIPART)
                        .body(multipart(&[
                            ("zpl", zpl.as_bytes()),
                            ("refresh", if refresh { b"true" } else { b"false" }),
                        ])),
                ];
                for request in requests {
                    let response = request.send().await.unwrap();
                    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{zpl:?}");
                    assert!(response.text().await.unwrap().starts_with("ZPL byte "));
                }
            }
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(identity_calls.load(Ordering::SeqCst), 0);
        use zpl_proxy_api::schema::{inputs, png_requests};
        assert_eq!(
            inputs::table
                .count()
                .get_result::<i64>(&mut connection)
                .unwrap(),
            1
        );
        assert_eq!(
            png_requests::table
                .count()
                .get_result::<i64>(&mut connection)
                .unwrap(),
            1
        );
        // Ordinary labels still reach the printer, then serve from the cache.
        for (index, request) in [
            client
                .post(&endpoint)
                .json(&serde_json::json!({"zpl": "^XA^FO10,20^FDhello^FS^XZ"})),
            client
                .post(&endpoint)
                .form(&[("zpl", "^XA^FO10,20^FDhello^FS^XZ")]),
            client
                .post(&endpoint)
                .header(header::CONTENT_TYPE, MULTIPART)
                .body(multipart(&[("zpl", b"^XA^FO10,20^FDhello^FS^XZ")])),
        ]
        .into_iter()
        .enumerate()
        {
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert!(!response.headers().contains_key("X-ZPL-Printer-Part-Number"));
            assert_eq!(
                response.headers()["X-ZPL-Cache"],
                if index == 0 { "miss" } else { "hit" }
            );
            assert_eq!(
                response.bytes().await.unwrap().as_ref(),
                png_metadata::annotate(
                    &png_metadata::test_png(),
                    &mock_identity(33),
                    "^XA^FO10,20^FDhello^FS^XZ",
                )
                .unwrap()
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(identity_calls.load(Ordering::SeqCst), 1);
        proxy_task.abort();
        printer_task.abort();
        sgd_server.abort();
    }

    async fn extract(content_type: &str, body: impl Into<Body>) -> Result<String, StatusCode> {
        let request = Request::builder()
            .method("POST")
            .header(header::CONTENT_TYPE, content_type)
            .body(body.into())
            .unwrap();
        JsonOrForm::from_request(request, &())
            .await
            .map(|JsonOrForm(spec)| spec.zpl)
            .map_err(|response| response.status())
    }

    fn multipart(fields: &[(&str, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, value) in fields {
            body.extend_from_slice(
                format!(
                    "--test-boundary\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(value);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--test-boundary--\r\n");
        body
    }
    const MULTIPART: &str = "multipart/form-data; boundary=test-boundary";

    #[tokio::test]
    async fn refresh_is_supported_in_all_request_formats() {
        for (content_type, body) in [
            (
                "application/json",
                br#"{"zpl":"^XA^XZ","refresh":true}"#.to_vec(),
            ),
            (
                "application/x-www-form-urlencoded",
                b"zpl=%5EXA%5EXZ&refresh=true".to_vec(),
            ),
            (
                MULTIPART,
                multipart(&[("zpl", b"^XA^XZ"), ("refresh", b"true")]),
            ),
        ] {
            let request = Request::builder()
                .method("POST")
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .unwrap();
            let JsonOrForm(spec) = JsonOrForm::from_request(request, &()).await.unwrap();
            assert!(spec.refresh);
        }
        assert_eq!(
            extract(
                MULTIPART,
                multipart(&[("zpl", b"^XA^XZ"), ("refresh", b"invalid")])
            )
            .await,
            Err(StatusCode::BAD_REQUEST)
        );
    }

    #[tokio::test]
    async fn accepts_supported_request_formats() {
        // Preserve all three existing endpoint formats after removing the derive.
        assert_eq!(
            extract("application/json", r#"{"zpl":"^XA^XZ"}"#)
                .await
                .unwrap(),
            "^XA^XZ"
        );
        assert_eq!(
            extract("application/x-www-form-urlencoded", "zpl=%5EXA%5EXZ")
                .await
                .unwrap(),
            "^XA^XZ"
        );
        assert_eq!(
            extract(MULTIPART, multipart(&[("zpl", b"^XA^XZ")]))
                .await
                .unwrap(),
            "^XA^XZ"
        );
    }

    #[tokio::test]
    async fn multipart_preserves_non_strict_field_semantics() {
        let body = multipart(&[
            ("ignored", b"value"),
            ("", b"nameless"),
            ("zpl", b"first"),
            ("zpl", b"last"),
        ]);
        assert_eq!(extract(MULTIPART, body).await.unwrap(), "last");
        assert_eq!(
            extract(MULTIPART, multipart(&[("zpl", b"")]))
                .await
                .unwrap(),
            ""
        );
        assert_eq!(
            extract(MULTIPART, multipart(&[("other", b"value")])).await,
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            extract(MULTIPART, multipart(&[("zpl", &[0xff])])).await,
            Err(StatusCode::BAD_REQUEST)
        );
    }

    #[tokio::test]
    async fn malformed_and_unsupported_bodies_are_rejected() {
        assert_eq!(
            extract("text/plain", "^XA^XZ").await,
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        );
        assert_eq!(
            extract("multipart/form-data", "bad").await,
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            extract(MULTIPART, "--test-boundary\r\n").await,
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            extract("application/json", "{").await,
            Err(StatusCode::BAD_REQUEST)
        );
    }

    #[tokio::test]
    async fn multipart_keeps_axum_body_limit() {
        // https://docs.rs/axum/0.8/axum/extract/struct.DefaultBodyLimit.html
        let value = vec![b'x'; 2 * 1024 * 1024 + 1];
        assert_eq!(
            extract(MULTIPART, multipart(&[("zpl", &value)])).await,
            Err(StatusCode::PAYLOAD_TOO_LARGE)
        );
    }
}
