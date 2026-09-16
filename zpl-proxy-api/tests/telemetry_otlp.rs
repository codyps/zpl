//! Exercise the real exporter against a local collector, never an external service.
use axum::{body::Bytes, http::StatusCode, routing::post, Router};
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exports_and_flushes_otlp_to_local_collector() {
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = received.clone();
    let app = Router::new().route(
        "/v1/traces",
        post(move |body: Bytes| async move {
            captured.lock().unwrap().extend_from_slice(&body);
            StatusCode::OK
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/traces", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    // This integration-test binary has one test, isolating global telemetry and env.
    std::env::set_var("NO_PROXY", "127.0.0.1");
    std::env::set_var("no_proxy", "127.0.0.1");
    std::env::set_var("OTEL_SDK_DISABLED", "false");
    std::env::set_var("OTEL_TRACES_EXPORTER", "otlp");
    std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", endpoint);
    std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_HEADERS", "");
    std::env::set_var("OTEL_SERVICE_NAME", "zpl-test-service");
    std::env::set_var("OTEL_BSP_SCHEDULE_DELAY", "10");
    let guard = zpl_proxy_api::telemetry::init().unwrap();
    drop(fastrace::Span::root(
        "otlp-smoke",
        fastrace::collector::SpanContext::random(),
    ));
    tokio::task::spawn_blocking(move || drop(guard))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if !received.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("collector received exported spans");
    let bytes = received.lock().unwrap();
    for expected in ["otlp-smoke", "zpl-test-service"] {
        assert!(bytes
            .windows(expected.len())
            .any(|part| part == expected.as_bytes()));
    }
    server.abort();
}
