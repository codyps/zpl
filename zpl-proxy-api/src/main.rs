mod listener;

use axum::{
    body::Body,
    error_handling::HandleError,
    extract::{Form, FromRequest, Json, Multipart, Path as ApiPath, State},
    http::{header, Request, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    RequestExt, Router,
};
use clap::Parser;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use zpl_proxy_api::{
    cache::Cache,
    printer::{Ownership, Printer, PrinterConfig, Rejected, Unavailable},
    telemetry,
    validation::ValidationError,
};

#[derive(Debug, Parser)]
struct Args {
    /// JSON array of named printers with url, control_address, width and height.
    #[clap(
        long,
        conflicts_with = "zd621_url",
        required_unless_present = "zd621_url"
    )]
    printers: Option<PathBuf>,
    /// Legacy single-printer configuration; public name is zd621.
    #[clap(long)]
    zd621_url: Option<reqwest::Url>,
    #[clap(long)]
    zd621_header: Vec<String>,
    #[command(flatten)]
    listen: listener::ListenOptions,
    #[clap(long, default_value = "default")]
    cache_namespace: String,
}

#[derive(Clone)]
struct AppState {
    printers: Arc<HashMap<String, Printer>>,
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

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let cache = tokio::task::spawn_blocking(move || Cache::open(&database_url)).await??;
    let configs: Vec<PrinterConfig> = if let Some(path) = args.printers {
        serde_json::from_slice(&std::fs::read(path)?)?
    } else {
        let url = args
            .zd621_url
            .expect("clap requires a printer configuration");
        let host = url
            .host_str()
            .ok_or_else(|| eyre::eyre!("missing printer host"))?;
        let control_address = if host.contains(':') {
            format!("[{host}]:9100")
        } else {
            format!("{host}:9100")
        };
        vec![PrinterConfig {
            name: "zd621".into(),
            url: url.to_string(),
            control_address,
            width: 832,
            height: 1218,
            headers: args.zd621_header,
            serial: None,
        }]
    };
    let app_state = configured_printers(configs, cache, &args.cache_namespace)?;
    let api_router = Router::new()
        .route("/printers", get(printer_names))
        .route("/printers/{name}/preview", post(named_zpl_to_png))
        .route("/zpl-zd621", post(zd621_zpl_to_png))
        .with_state(app_state);

    let app = Router::new()
        .nest("/api", api_router)
        // Axum 0.8 rejects nesting at "/"; use the static service as fallback.
        // https://docs.rs/axum/0.8.9/axum/struct.Router.html#method.fallback_service
        .fallback_service(HandleError::new(
            tower_http::services::ServeDir::new(Path::new("assets")),
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

fn configured_printers(
    configs: Vec<PrinterConfig>,
    cache: Cache,
    namespace: &str,
) -> eyre::Result<AppState> {
    eyre::ensure!(!configs.is_empty(), "configure at least one printer");
    let mut printers = HashMap::new();
    let owners = Ownership::default();
    let mut endpoints = std::collections::HashSet::new();
    let mut controls = std::collections::HashSet::new();
    for config in configs {
        let url: reqwest::Url = config.url.parse()?;
        eyre::ensure!(
            endpoints.insert(url.to_string()) && controls.insert(config.control_address.clone()),
            "duplicate printer endpoint; configure each physical printer once"
        );
        let printer = Printer::new(config, cache.clone(), namespace, owners.clone())?;
        eyre::ensure!(
            printers.insert(printer.name().into(), printer).is_none(),
            "duplicate printer name"
        );
    }
    Ok(AppState {
        printers: Arc::new(printers),
    })
}

async fn printer_names(State(state): State<AppState>) -> Json<Vec<String>> {
    let mut names = state.printers.keys().cloned().collect::<Vec<_>>();
    names.sort();
    Json(names)
}

async fn named_zpl_to_png(
    State(state): State<AppState>,
    ApiPath(name): ApiPath<String>,
    JsonOrForm(spec): JsonOrForm,
) -> Response<Body> {
    render_response(state, name, spec).await
}

async fn zd621_zpl_to_png(
    State(state): State<AppState>,
    JsonOrForm(spec): JsonOrForm,
) -> Response<Body> {
    render_response(state, "zd621".into(), spec).await
}

async fn render_response(state: AppState, name: String, spec: PrintSpec) -> Response<Body> {
    let Some(printer) = state.printers.get(&name) else {
        return (StatusCode::NOT_FOUND, "Unknown printer name").into_response();
    };
    match printer.render(spec.zpl, spec.refresh).await {
        Ok((png, cache_hit)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header("X-ZPL-Cache", if cache_hit { "hit" } else { "miss" })
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from(png))
            .unwrap(),
        Err(error) if error.downcast_ref::<ValidationError>().is_some() => (
            StatusCode::BAD_REQUEST,
            error.downcast_ref::<ValidationError>().unwrap().to_string(),
        )
            .into_response(),
        Err(error) if error.downcast_ref::<Unavailable>().is_some() => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "300")],
            "Printer unavailable; retry later",
        )
            .into_response(),
        Err(error) if error.downcast_ref::<Rejected>().is_some() => {
            let cached = error.downcast_ref::<Rejected>().unwrap().cached;
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                [("X-ZPL-Cache", if cached { "hit" } else { "miss" })],
                "Label repeatedly failed on this printer firmware",
            )
                .into_response()
        }
        Err(_) => {
            log::error!("render or cache operation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to render and persist the label",
            )
                .into_response()
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
    #[tokio::test]
    async fn named_routes_reject_unknown_printers_and_unsafe_history_in_every_format() {
        use diesel::{connection::SimpleConnection, prelude::*};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let mut db = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        for migration in [
            include_str!("../migrations/2024-10-03-035443_cache-results/up.sql"),
            include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql"),
            include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
            include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
            include_str!("../migrations/2026-09-29-120000_printer-management/up.sql"),
        ] {
            db.batch_execute(migration).unwrap();
        }
        let cache = Cache::open(path.to_str().unwrap()).unwrap();
        let forbidden = "^XA^XGR:PRIVATE.GRF,1,1^FS^XZ";
        let attempt = cache
            .begin(forbidden.as_bytes().to_vec(), vec![1], false)
            .await
            .unwrap();
        cache
            .success(attempt, b"private PNG".to_vec())
            .await
            .unwrap();
        let configs = vec![PrinterConfig {
            name: "ZD621-V93".into(),
            url: "http://127.0.0.1:9/".into(),
            control_address: "127.0.0.1:9".into(),
            width: 832,
            height: 1218,
            headers: vec![],
            serial: None,
        }];
        let app = Router::new()
            .route("/api/printers/{name}/preview", post(named_zpl_to_png))
            .with_state(configured_printers(configs, cache, "test").unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/api/printers", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let unknown = client
            .post(format!("{base}/unknown/preview"))
            .json(&serde_json::json!({"zpl":"^XA^XZ"}))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
        let endpoint = format!("{base}/ZD621-V93/preview");
        for zpl in [
            forbidden,
            "^XA^FO1,2^FDhello^FS^XZ^",
            "^XA^WD*:*.*^XZ",
            "^XA^FXcomment\n! U1 getvar \"allcv\"\n^FS^XZ",
        ] {
            for refresh in [false, true] {
                for request in [
                    client
                        .post(&endpoint)
                        .json(&serde_json::json!({"zpl":zpl,"refresh":refresh})),
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
                ] {
                    assert_eq!(
                        request.send().await.unwrap().status(),
                        StatusCode::BAD_REQUEST
                    );
                }
            }
        }
        use zpl_proxy_api::schema::png_requests;
        assert_eq!(
            png_requests::table
                .count()
                .get_result::<i64>(&mut db)
                .unwrap(),
            1
        );
        server.abort();
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
