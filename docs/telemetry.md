# Proxy telemetry

The proxy uses native **fastrace** spans and **log + Logforth** logging, following
the Beachout setup. It has no direct `tracing` or `tracing-subscriber` dependency
and installs no compatibility subscriber. Some dependencies still use `tracing`
internally; their spans are not bridged.

HTTP middleware creates a request span, adopting a valid W3C `traceparent` or
starting a new trace. Incoming sampling flags are honored. Logs include the
current trace/span context and are also attached to the span as events. Correlation
stays in telemetry: no trace IDs are written to SQLite or added to responses.

Child spans cover rendering, the printer queue and HTTP preview, and cache
lookup/store/failure operations. Context crosses both Tokio task and blocking
database thread boundaries. Request duration ends when the response is created,
not when its body finishes streaming. Graceful Ctrl-C/SIGTERM shutdown drains
active HTTP requests and flushes collected spans.

Instrumentation records method, matched route template, response status, cache
hit/miss, and database operation timing. It does not attach raw URLs, query
strings, IPs, headers, SQL/bind values, ZPL, or PNG bytes. Error logs use stable
messages; detailed printer outcomes remain in the existing cache records.

## Configuration

- `RUST_LOG`: log filtering; defaults to `info`.
- `OTEL_TRACES_EXPORTER=console` (default): print spans to stderr; no collector required.
- `OTEL_TRACES_EXPORTER=otlp`: export spans over OTLP HTTP/protobuf. Set
  `OTEL_EXPORTER_OTLP_ENDPOINT` (for example `http://localhost:4318`) or
  `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` for an explicit trace endpoint.
- `OTEL_EXPORTER_OTLP_HEADERS` / `OTEL_EXPORTER_OTLP_TRACES_HEADERS`: optional
  exporter authentication supplied through the environment, never repository files.
- `OTEL_SERVICE_NAME` / `OTEL_RESOURCE_ATTRIBUTES`: exporter resource metadata;
  service name defaults to `zpl-proxy-api`.
- `OTEL_BSP_SCHEDULE_DELAY`: positive reporting interval in milliseconds (default 1000).
- `OTEL_TRACES_EXPORTER=none` or `OTEL_SDK_DISABLED=true`: disable span reporting;
  ordinary stderr logging remains enabled.

No external telemetry backend is contacted unless OTLP export is selected.
This setup exports traces, not a separate metrics or OTLP-log pipeline.

References:

- [fastrace async propagation](https://docs.rs/fastrace/0.7.19/fastrace/future/index.html)
- [Logforth](https://github.com/fast/logforth)
- [OTLP exporter configuration](https://docs.rs/opentelemetry-otlp/0.32.0/opentelemetry_otlp/)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
