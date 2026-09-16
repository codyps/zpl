use super::*;
use axum::{http::StatusCode, middleware, routing::post, Json, Router};
use fastrace::collector::TestReporter;

#[test]
fn exporter_configuration_is_explicit() {
    assert_eq!(exporter("console").unwrap(), Exporter::Console);
    assert_eq!(exporter("otlp").unwrap(), Exporter::Otlp);
    assert_eq!(exporter(" none ").unwrap(), Exporter::None);
    assert!(exporter("typo").is_err());
}

#[tokio::test]
async fn middleware_correlates_logs_and_async_and_blocking_tasks_without_payloads() {
    // There is one global reporter/logger in this test process. Other unit tests
    // have no request roots, and do not configure telemetry.
    init_logging();
    let (reporter, records) = TestReporter::new();
    fastrace::set_reporter(reporter, Config::default());
    let app = Router::new()
        .route(
            "/labels/{id}",
            post(|Json(_): Json<serde_json::Value>| async {
                spawn("test.render", async {
                    tokio::task::yield_now().await;
                    spawn_blocking("test.database", || {
                        assert!(SpanContext::current_local_parent().is_some());
                        log::error!("test render failure");
                    })
                    .await
                    .unwrap();
                })
                .await
                .unwrap();
                StatusCode::SERVICE_UNAVAILABLE
            }),
        )
        .layer(middleware::from_fn(request));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/labels/private-label?secret=private-query",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let context = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let response = client
        .post(&url)
        .header("traceparent", context)
        .header("Authorization", "private-auth")
        .json(&serde_json::json!({"zpl":"private-zpl"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.headers().get("X-Trace-Id").is_none());
    // Extractor rejection still completes the request span. Invalid context
    // starts a new trace; the remote sampled=0 flag disables collection.
    assert_eq!(
        client
            .post(&url)
            .header("traceparent", "invalid")
            .header("Content-Type", "application/json")
            .body("{")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    client
        .post(&url)
        .header(
            "traceparent",
            "00-11111111111111111111111111111111-00f067aa0ba902b7-00",
        )
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    server.abort();
    fastrace::flush();
    let records = records.lock();
    let expected = SpanContext::decode_w3c_traceparent(context).unwrap();
    let root = records
        .iter()
        .find(|r| r.name == "http.request" && r.trace_id == expected.trace_id)
        .unwrap();
    assert_eq!(root.parent_id, expected.span_id);
    assert!(root
        .properties
        .iter()
        .any(|(k, v)| k == "http.route" && v == "/labels/{id}"));
    assert!(root
        .properties
        .iter()
        .any(|(k, v)| k == "http.response.status_code" && v == "503"));
    assert!(root
        .properties
        .iter()
        .any(|(k, v)| k == "span.status_code" && v == "error"));
    let render = records
        .iter()
        .find(|r| r.name == "test.render" && r.trace_id == expected.trace_id)
        .unwrap();
    let database = records
        .iter()
        .find(|r| r.name == "test.database" && r.trace_id == expected.trace_id)
        .unwrap();
    assert_eq!(render.parent_id, root.span_id);
    assert_eq!(database.parent_id, render.span_id);
    assert!(database
        .events
        .iter()
        .any(|e| format!("{e:?}").contains("test render failure")));
    assert!(records.iter().any(|r| r.name == "http.request"
        && r.properties
            .iter()
            .any(|(k, v)| k == "http.response.status_code" && v == "400")));
    assert!(!records
        .iter()
        .any(|r| r.trace_id.0 == 0x11111111111111111111111111111111));
    let output = format!("{records:?}");
    for private in [
        "private-label",
        "private-query",
        "private-auth",
        "private-zpl",
        "127.0.0.1",
    ] {
        assert!(!output.contains(private), "telemetry leaked {private}");
    }
}
