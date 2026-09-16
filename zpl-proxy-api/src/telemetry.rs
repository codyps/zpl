//! Request tracing is middleware, not part of the cache schema.
//! Async propagation: https://docs.rs/fastrace/0.7.19/fastrace/future/index.html
//! W3C context: https://www.w3.org/TR/trace-context/#traceparent-header
//! Logging follows Beachout's fastrace + log/Logforth setup.
use axum::{
    body::Body,
    extract::MatchedPath,
    http::{Request, Response},
    middleware::Next,
};
use fastrace::{
    collector::{Config, ConsoleReporter, SpanContext},
    future::FutureExt,
    local::LocalSpan,
    Span,
};
use std::{borrow::Cow, future::Future, time::Duration};

pub struct TelemetryGuard;

#[cfg(test)]
mod tests;

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        fastrace::flush();
    }
}

#[derive(Debug, PartialEq)]
enum Exporter {
    Console,
    Otlp,
    None,
}

fn exporter(value: &str) -> eyre::Result<Exporter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "console" => Ok(Exporter::Console),
        "otlp" => Ok(Exporter::Otlp),
        "none" => Ok(Exporter::None),
        _ => {
            eyre::bail!("OTEL_TRACES_EXPORTER must be console, otlp, or none");
        }
    }
}

fn init_logging() {
    let filter =
        logforth::filter::rustlog::RustLogFilterBuilder::from_default_env_or("info").build();
    logforth::starter_log::builder()
        .dispatch(|dispatch| {
            dispatch
                .filter(filter)
                .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
                .append(logforth::append::Stderr::default())
                .append(logforth::append::FastraceEvent::default())
        })
        .apply();
}

pub fn init() -> eyre::Result<TelemetryGuard> {
    init_logging();
    let disabled = std::env::var("OTEL_SDK_DISABLED")
        .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1"));
    let mode = if disabled {
        Exporter::None
    } else {
        exporter(&std::env::var("OTEL_TRACES_EXPORTER").unwrap_or_else(|_| "console".into()))?
    };
    let interval = std::env::var("OTEL_BSP_SCHEDULE_DELAY")
        .ok()
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(1000);
    eyre::ensure!(
        interval > 0,
        "OTEL_BSP_SCHEDULE_DELAY must be positive milliseconds"
    );
    let config = Config::default().report_interval(Duration::from_millis(interval));
    match mode {
        Exporter::Console => fastrace::set_reporter(ConsoleReporter, config),
        Exporter::None => {}
        Exporter::Otlp => {
            // OTLP is opt-in; endpoint and authentication come from OTEL_* env.
            // https://docs.rs/opentelemetry-otlp/0.32.0/opentelemetry_otlp/
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .build()?;
            let detected = opentelemetry_sdk::Resource::builder().build();
            let service_name = opentelemetry::Key::from_static_str("service.name");
            let fallback = detected
                .get(&service_name)
                .is_none_or(|v| v.as_str().starts_with("unknown_service"));
            let resource = if fallback {
                opentelemetry_sdk::Resource::builder()
                    .with_service_name("zpl-proxy-api")
                    .build()
            } else {
                detected
            };
            let scope = opentelemetry::InstrumentationScope::builder("zpl-proxy-api")
                .with_version(env!("CARGO_PKG_VERSION"))
                .build();
            fastrace::set_reporter(
                fastrace_opentelemetry::OpenTelemetryReporter::new(
                    exporter,
                    Cow::Owned(resource),
                    scope,
                ),
                config,
            );
        }
    }
    Ok(TelemetryGuard)
}

/// This span measures handling through response creation, not response streaming.
pub async fn request(req: Request<Body>, next: Next) -> Response<Body> {
    let context = req
        .headers()
        .get("traceparent")
        .and_then(|v| v.to_str().ok())
        .and_then(SpanContext::decode_w3c_traceparent)
        .unwrap_or_else(SpanContext::random);
    // Matched route templates only: never raw paths, queries, headers, IPs or ZPL.
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("<unmatched>")
        .to_owned();
    let method = req.method().to_string();
    let span = Span::root("http.request", context).with_properties(|| {
        [
            ("span.kind", "server".to_owned()),
            ("http.request.method", method),
            ("http.route", route),
        ]
    });
    async {
        let response = next.run(req).await;
        let status = response.status();
        LocalSpan::add_property(|| ("http.response.status_code", status.as_u16().to_string()));
        if status.is_server_error() {
            LocalSpan::add_property(|| ("span.status_code", "error"));
        }
        log::info!("HTTP request completed with status {}", status.as_u16());
        response
    }
    .in_span(span)
    .await
}

/// Capture parent context before Tokio switches tasks.
pub fn spawn<F>(name: &'static str, future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future.in_span(Span::enter_with_local_parent(name)))
}

/// A synchronous local-parent guard is safe here: it never crosses an await.
pub fn spawn_blocking<F, T>(name: &'static str, work: F) -> tokio::task::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let span = Span::enter_with_local_parent(name);
    tokio::task::spawn_blocking(move || {
        let _parent = span.set_local_parent();
        work()
    })
}

pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.expect("install Ctrl-C handler"),
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("install Ctrl-C handler");
}
