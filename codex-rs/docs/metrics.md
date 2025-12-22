# Metrics (Statsig HTTP)

The `codex_otel::metrics` module sends counters and histograms to a Statsig
backend by POSTing JSON to the Statsig `log_event` endpoint. A tokio-backed
worker keeps callers non-blocking while metrics are serialized and sent.

Defaults are provided for the Statsig API key, header name, and endpoint so
you can send metrics immediately. Override them if you need to target a
different Statsig project.

## Quick start

```rust
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;

let metrics = MetricsClient::new(
    MetricsConfig::new("<statsig-api-key>")
        .with_endpoint("<statsig-log-event-endpoint>")
        .with_api_key_header("<statsig-api-key-header>")
        .with_tag("service", "codex-cli")?,
)?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

## OtelManager facade

If you're already using `OtelManager` for tracing, you can attach a metrics
client and emit metrics through the same handle. By default, metrics sent via
`OtelManager` include metadata tags: `auth_mode`, `model`, `slug`,
`terminal.type`, and `app.version`. Use
`with_metrics_without_metadata_tags` to opt out.

```rust
use codex_otel::metrics::MetricsConfig;
use codex_otel::OtelManager;

let manager = OtelManager::new(
    conversation_id,
    model,
    slug,
    account_id,
    account_email,
    auth_mode,
    log_user_prompts,
    terminal_type,
    session_source,
)
.with_metrics_config(
    MetricsConfig::new("<statsig-api-key>")
        .with_endpoint("<statsig-log-event-endpoint>")
        .with_api_key_header("<statsig-api-key-header>"),
)?;
manager.counter("codex.session_started", 1, &[("source", "tui")])?;
manager.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

If you set `metrics: Some(MetricsConfig)` on `OtelSettings` and build an
`OtelProvider`, you can reuse that client via
`OtelManager::with_provider_metrics(&provider)`.

## Configuration

`MetricsConfig` lets you specify:

- `MetricsConfig::new(api_key)` to set the Statsig API key.
- `with_endpoint(endpoint)` to set the Statsig `log_event` endpoint.
- `with_api_key_header(header)` to set the API key header name.
- `with_tag(key, value)` to add default tags for every metric.
- `with_timeout(duration)` to set the HTTP request timeout.
- `with_export_interval(duration)` to tweak the in-memory exporter interval in tests.
- `with_user_agent(agent)` to override the HTTP `User-Agent` header.

The queue capacity is fixed at 1024 entries.

## Timing

Measure a closure and emit a histogram sample for the elapsed time in
milliseconds:

```rust
let result = metrics.time("codex.request_latency", &[("route", "chat")], || {
    "ok"
})?;
```

If the closure already returns `codex_otel::metrics::Result<T>`, use
`time_result` to avoid nested results:

```rust
let result = metrics.time_result(
    "codex.request_latency",
    &[("route", "chat")],
    || Ok("ok"),
)?;
```

If you already have a duration, record it directly:

```rust
metrics.record_duration(
    "codex.request_latency",
    std::time::Duration::from_millis(83),
    &[("route", "chat")],
)?;
```

## Shutdown and queue capacity

The client uses a bounded queue (default capacity 1024). Enqueueing returns a
`MetricsError::QueueFull` error if the queue is full or
`MetricsError::WorkerUnavailable` if the worker is no longer running.

`shutdown` flushes queued metrics, requests a final export, and waits up to
500ms for the worker to stop. `MetricsClient` also attempts a best-effort
shutdown on drop using the default timeout, so explicit calls to `shutdown`
are optional.

## Validation rules

Metric names:

- Must be non-empty.
- Allowed characters: ASCII letters/digits plus `.`, `_`, `-`.

Tag keys and values:

- Must be non-empty.
- Allowed characters: ASCII letters/digits plus `.`, `_`, `-`, `/`.
- The tag key `le` is reserved.

## Error handling

All APIs return `codex_otel::metrics::Result<T>` with a `MetricsError` variant
on failure. Errors cover invalid configuration, validation failures, queue
backpressure, and HTTP client setup or request failures.
