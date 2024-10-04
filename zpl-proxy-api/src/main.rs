use std::sync::Arc;

use axum::{
    async_trait,
    body::Body,
    extract::FromRequest,
    extract::{Form, Json, State},
    http::{self, header, HeaderName, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    routing::post,
    RequestExt, Router,
};
use axum_typed_multipart::TypedMultipart;
use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Parser)]
struct Args {
    #[clap(long)]
    zd621_url: reqwest::Url,
    #[clap(long)]
    zd621_header: Vec<String>,
    #[clap(long)]
    bind_addr: std::net::SocketAddr,
}

struct AppState {
    zd621_client: reqwest::Client,
    zd621_url: reqwest::Url,
    db: r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::SqliteConnection>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let zd621_client = reqwest::Client::builder()
        .default_headers(
            args.zd621_header
                .iter()
                .map(|header| {
                    let mut parts = header.splitn(2, ": ");
                    let key = parts.next().unwrap();
                    let value = parts.next().unwrap();
                    let key: HeaderName = key.parse().unwrap();
                    let value: HeaderValue = value.parse().unwrap();
                    (key, value)
                })
                .collect(),
        )
        .build()
        .unwrap();

    let db_path = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = diesel::r2d2::ConnectionManager::<diesel::SqliteConnection>::new(db_path);
    // FIXME: blocking?
    let db = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let app_state = Arc::new(AppState {
        zd621_client,
        zd621_url: args.zd621_url,
        db,
    });

    let app = Router::new()
        .route("/zpl-zd621", post(zd621_zpl_to_png))
        .with_state(app_state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(args.bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize, axum_typed_multipart::TryFromMultipart)]
struct PrintSpec {
    zpl: String,
}

async fn zd621_zpl_to_png(
    State(state): State<Arc<AppState>>,
    JsonOrForm(print_spec): JsonOrForm<PrintSpec>,
) -> impl IntoResponse {
    // TODO: parse zpl, check that it contains only commands we allow.
    // TODO: modify zpl to set fixed initial state
    // TODO: submit modified zpl to zd621 api
    // TODO: return png

    let png = match zebra_http_api::zpl_to_png(
        state.zd621_client.clone(),
        state.zd621_url.clone(),
        &print_spec.zpl,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("zd621 api error: {}", e);
            return Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("zd621 api error: {}", e)))
                .unwrap();
        }
    };

    // FIXME: save zpl and png to db
    diesel::inse

    tracing::info!("zpl to png conversion successful");
    Response::builder()
        .status(http::StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(png))
        .unwrap()
}

struct JsonOrForm<T>(T);

#[async_trait]
impl<S, T> FromRequest<S> for JsonOrForm<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<()>,
    Form<T>: FromRequest<()>,
    TypedMultipart<T>: FromRequest<()>,
    T: 'static,
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
                let TypedMultipart(payload) =
                    req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }
        }

        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}
