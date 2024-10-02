use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::{self, header, HeaderName, HeaderValue},
    response::IntoResponse,
    routing::post,
    Router,
};
use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Parser)]
struct Args {
    zd621_url: reqwest::Url,
    zd621_header: Vec<String>,
    bind_addr: std::net::SocketAddr,
}

struct AppState {
    zd621_client: reqwest::Client,
    zd621_url: reqwest::Url,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let zd621_client = reqwest::Client::builder()
        .default_headers(
            args.zd621_header
                .iter()
                .map(|header| {
                    let mut parts = header.splitn(2, ':');
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

    let app_state = Arc::new(AppState {
        zd621_client,
        zd621_url: args.zd621_url,
    });

    let app = Router::new()
        .route("/zpl-zd621", post(zd621_zpl_to_png))
        .with_state(app_state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(args.bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct PrintSpec {
    zpl: String,
}

async fn zd621_zpl_to_png(
    State(state): State<Arc<AppState>>,
    Json(print_spec): Json<PrintSpec>,
) -> impl IntoResponse {
    // TODO: parse zpl, check that it contains only commands we allow.
    // TODO: modify zpl to set fixed initial state
    // TODO: submit modified zpl to zd621 api
    // TODO: return png

    let png = zebra_http_api::zpl_to_png(
        state.zd621_client.clone(),
        state.zd621_url.clone(),
        &print_spec.zpl,
    )
    .await
    .map_err(|e| {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("zd621 api error: {}", e),
        )
    })?;

    Ok((
        http::StatusCode::OK,
        &[(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("image/png"),
        )][..],
        png,
    ))
}
