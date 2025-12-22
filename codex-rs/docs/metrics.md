# Metrics (Statsig + OTEL)

The `codex_otel::metrics` module sends counters and histograms to a Statsig
backend using OTLP/HTTP. It uses a background worker to keep callers
non-blocking and exports metrics via OpenTelemetry.

You must supply a Statsig OTLP endpoint and API key. This module ships with
placeholders (`<statsig-otlp-metrics-endpoint>`, `<statsig-api-key-header>`,
`<statsig-api-key>`) so they are obvious to replace.

## Quick start

```rust
use codex_otel::metrics::HistogramBuckets;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;

let metrics = MetricsClient::new(
    MetricsConfig::new("<statsig-api-key>")
        .with_endpoint("<statsig-otlp-metrics-endpoint>")
        .with_api_key_header("<statsig-api-key-header>")
        .with_tag("service", "codex-cli")?,
)?;

let buckets = HistogramBuckets::from_values(&[25, 50, 100, 250, 500, 1000])?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &buckets, &[("route", "chat")])?;
```

## OtelManager facade

If you're already using `OtelManager` for tracing, you can attach a metrics
client and emit metrics through the same handle. By default, metrics sent via
`OtelManager` include metadata tags: `auth_mode`, `model`, `slug`,
`terminal.type`, and `app.version`. Use
`with_metrics_without_metadata_tags` to opt out.

```rust
use codex_otel::metrics::HistogramBuckets;
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
        .with_endpoint("<statsig-otlp-metrics-endpoint>")
        .with_api_key_header("<statsig-api-key-header>"),
)?;

let buckets = HistogramBuckets::from_values(&[25, 50, 100, 250, 500])?;
manager.counter("codex.session_started", 1, &[("source", "tui")])?;
manager.histogram("codex.request_latency", 83, &buckets, &[("route", "chat")])?;
```

If you set `metrics: Some(MetricsConfig)` on `OtelSettings` and build an
`OtelProvider`, you can reuse that client via
`OtelManager::with_provider_metrics(&provider)`.

## Configuration

`MetricsConfig` lets you specify:

- `MetricsConfig::new(api_key)` to set the Statsig API key.
- `with_endpoint(endpoint)` to set the OTLP endpoint.
- `with_api_key_header(header)` to set the API key header name.
- `with_tag(key, value)` to add default tags for every metric.
- `with_timeout(duration)` to set the OTLP export timeout.
- `with_export_interval(duration)` to set the periodic export interval.
- `with_user_agent(agent)` to override the HTTP `User-Agent` header.

The queue capacity is fixed at 1024 entries.

## Histograms

Histograms are recorded as OpenTelemetry histograms. Bucket boundaries are
controlled by the OTEL pipeline (collector/exporter configuration). The
`HistogramBuckets` type is retained for API compatibility and validation but
is not used to pre-bucket samples.

## Timing

Measure a closure and emit a histogram sample for the elapsed time in
milliseconds:

```rust
let result = metrics.time("codex.request_latency", &buckets, &[("route", "chat")], || {
    "ok"
})?;
```

If the closure already returns `codex_otel::metrics::Result<T>`, use
`time_result` to avoid nested results:

```rust
let result = metrics.time_result(
    "codex.request_latency",
    &buckets,
    &[("route", "chat")],
    || Ok("ok"),
)?;
```

If you already have a duration, record it directly:

```rust
metrics.record_duration(
    "codex.request_latency",
    std::time::Duration::from_millis(83),
    &buckets,
    &[("route", "chat")],
)?;
```

## Batching

Batching reduces overhead and keeps metrics aligned in time:

```rust
let mut batch = metrics.batch();
batch.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
batch.histogram("codex.tool_latency", 140, &buckets, &[("tool", "shell")])?;
metrics.send(batch)?;
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
backpressure, and OTLP exporter setup issues.
