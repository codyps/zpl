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
    collections::{BTreeMap, HashMap},
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
    /// JSON object keyed by printer name, with url, control_address, width and height.
    #[clap(long)]
    printers: PathBuf,
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
    let configs = parse_printers(&std::fs::read(args.printers)?)?;
    let app_state = configured_printers(configs, cache, &args.cache_namespace)?;
    let api_router = Router::new()
        .route("/printers", get(printer_names))
        .route("/printers/{name}/preview", post(named_zpl_to_png))
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

fn parse_printers(bytes: &[u8]) -> eyre::Result<BTreeMap<String, PrinterConfig>> {
    // Serde errors can quote invalid values. Configuration may contain secrets,
    // so report only the location, never parser-provided input excerpts.
    #[derive(Deserialize)]
    struct Config(#[serde(deserialize_with = "unique_printers")] BTreeMap<String, PrinterConfig>);
    serde_json::from_slice::<Config>(bytes)
        .map(|config| config.0)
        .map_err(|error| {
            eyre::eyre!(
                "Invalid printer configuration at line {}, column {}",
                error.line(),
                error.column()
            )
        })
}

// JSON permits repeated object keys; reject them instead of silently replacing a
// printer (Serde MapAccess: https://serde.rs/deserialize-map.html).
fn unique_printers<'de, D>(deserializer: D) -> Result<BTreeMap<String, PrinterConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = BTreeMap<String, PrinterConfig>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an object keyed by unique printer names")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut printers = BTreeMap::new();
            while let Some((name, config)) = map.next_entry()? {
                if printers.insert(name, config).is_some() {
                    return Err(serde::de::Error::custom("duplicate printer name"));
                }
            }
            Ok(printers)
        }
    }
    deserializer.deserialize_map(Visitor)
}

fn configured_printers(
    configs: BTreeMap<String, PrinterConfig>,
    cache: Cache,
    namespace: &str,
) -> eyre::Result<AppState> {
    eyre::ensure!(!configs.is_empty(), "configure at least one printer");
    let mut printers = HashMap::new();
    let owners = Ownership::default();
    let mut endpoints = std::collections::HashSet::new();
    let mut controls = std::collections::HashSet::new();
    for (name, config) in configs {
        let url: reqwest::Url = config.url.parse()?;
        eyre::ensure!(
            endpoints.insert(url.to_string()) && controls.insert(config.control_address.clone()),
            "duplicate printer endpoint; configure each physical printer once"
        );
        let printer = Printer::new(
            name.clone(),
            config,
            cache.clone(),
            namespace,
            owners.clone(),
        )?;
        printers.insert(name, printer);
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
    #[test]
    fn configuration_is_keyed_by_public_name() {
        let config = r#"{"ZD621":{"url":"http://one.local/","control_address":"one.local:9100","width":832,"height":1218},"ZQ610":{"url":"http://two.local/","control_address":"two.local:9100","width":384,"height":600}}"#;
        let printers = parse_printers(config.as_bytes()).unwrap();
        assert_eq!(printers.len(), 2);
        assert_eq!(printers["ZD621"].width, 832);
        assert_eq!(printers["ZQ610"].url, "http://two.local/");
    }

    #[test]
    fn admission_is_selected_by_endpoint_configuration() {
        use zpl_proxy_api::validation::AdmissionPolicy;
        let base = r#""url":"http://printer.local/","control_address":"printer.local:9100","width":64,"height":32"#;
        let configs = parse_printers(
            format!(r#"{{"restricted":{{{base}}},"raw":{{{base},"admission":"unrestricted"}}}}"#)
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(configs["restricted"].admission, AdmissionPolicy::Restricted);
        assert_eq!(configs["raw"].admission, AdmissionPolicy::Unrestricted);
        assert!(
            parse_printers(format!(r#"{{"bad":{{{base},"admission":"typo"}}}}"#).as_bytes())
                .is_err()
        );
    }

    #[test]
    fn configuration_rejects_duplicate_keys_and_legacy_names() {
        let printer = r#"{"url":"http://printer.local/","control_address":"printer.local:9100","width":64,"height":32}"#;
        for config in [
            format!(r#"{{"Test":{printer},"Test":{printer}}}"#),
            format!("[{printer}]"),
            format!(
                r#"{{"Test":{{"name":"Test",{}}}}}"#,
                &printer[1..printer.len() - 1]
            ),
        ] {
            assert!(parse_printers(config.as_bytes()).is_err());
        }
    }

    #[test]
    fn configuration_errors_do_not_disclose_secret_values() {
        let secret = "PRIVATE_AUTH_TOKEN";
        let bytes = format!(
            r#"{{"Test":{{"url":"http://printer.local/","control_address":"printer.local:9100","width":"{secret}","height":32}}}}"#
        );
        let error = parse_printers(bytes.as_bytes()).err().unwrap();
        assert!(error.to_string().contains("line 1, column"));
        assert!(!format!("{error:?}").contains(secret));
    }

    #[tokio::test]
    async fn named_routes_reject_unknown_printers_and_unsafe_history_in_every_format() {
        use diesel::prelude::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let mut db = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        let cache = Cache::open(path.to_str().unwrap()).unwrap();
        let forbidden = "^XA^XFR:PRIVATE.ZPL^FS^XZ";
        let attempt = cache
            .begin(forbidden.as_bytes().to_vec(), vec![1], false)
            .await
            .unwrap();
        cache
            .success(attempt, b"private PNG".to_vec())
            .await
            .unwrap();
        let configs = BTreeMap::from([(
            "ZD621-V93".into(),
            PrinterConfig {
                url: "http://127.0.0.1:9/".into(),
                control_address: "127.0.0.1:9".into(),
                width: 832,
                height: 1218,
                headers: vec![],
                serial: None,
                admission: Default::default(),
            },
        )]);
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

    #[tokio::test]
    async fn unrestricted_http_formats_bypass_zpl_admission_but_keep_history() {
        use diesel::prelude::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let cache = Cache::open(path.to_str().unwrap()).unwrap();
        let mut db = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        // Keep a bound, non-listening port so identity fails immediately and
        // deterministically after admission, without any physical printer access.
        let unavailable = tokio::net::TcpSocket::new_v4().unwrap();
        unavailable.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = unavailable.local_addr().unwrap();
        let configs = parse_printers(format!(r#"{{"raw":{{"url":"http://{address}/","control_address":"{address}","width":64,"height":32,"admission":"unrestricted"}}}}"#).as_bytes()).unwrap();
        let app = Router::new()
            .route("/api/printers/{name}/preview", post(named_zpl_to_png))
            .with_state(configured_printers(configs, cache, "test").unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/api/printers/raw/preview",
            listener.local_addr().unwrap()
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let input = "~HS\nnot a framed label";
        for request in [
            client
                .post(&endpoint)
                .json(&serde_json::json!({"zpl":input})),
            client.post(&endpoint).form(&[("zpl", input)]),
            client
                .post(&endpoint)
                .header(header::CONTENT_TYPE, MULTIPART)
                .body(multipart(&[("zpl", input.as_bytes())])),
        ] {
            // A transport failure (503), not ZPL admission rejection (400).
            assert_eq!(
                request.send().await.unwrap().status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        }
        use zpl_proxy_api::schema::png_requests as r;
        assert_eq!(
            r::table
                .filter(r::error.is_not_null())
                .filter(r::completed_at.is_not_null())
                .count()
                .get_result::<i64>(&mut db)
                .unwrap(),
            3
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
