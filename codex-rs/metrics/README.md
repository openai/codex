# codex-metrics

Send lightweight counters and histogram buckets to Sentry via the statsd envelope item.

Key points:
- Non-blocking for the sender. Metrics are processed by a dedicated worker.
- Tag validation and metric name validation are enforced before send to match Sentry requirements.

## Quick start

```rust
let metrics = MetricsClient::new(
    MetricsConfig::default() // Default to the standard Sentry DSN.
        .with_tag("service", "codex-cli")?,
)?;

let buckets = HistogramBuckets::from_values(&[25, 50, 100, 250, 500, 1000])?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &buckets, &[("route", "chat")])?;
```

## Configuration

`MetricsConfig` lets you specify:

- `MetricsConfig::new(dsn)` to set the Sentry DSN.
- `with_tag(key, value)` to add default tags.
- `with_timeout(duration)` to override the HTTP timeout (default 10s).
- `with_user_agent(agent)` to override the user agent.

The queue capacity is fixed at 1024 entries.

## Buckets

`HistogramBuckets` supports:
- `from_values(&[...])` for explicit upper bounds.
- `from_range(from, to, step)` to build linear buckets. Requires `step > 0` and `from <= to`. The upper bound is always included.
- `from_exponential(from, to, factor)` to build exponential buckets. Requires `from > 0`, `from <= to`, and a finite `factor > 1`. The upper bound is always included.

## Sending metrics

Counters send a single statsd counter increment with tags:

```rust
metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
```

Histograms are translated into bucket counters by adding an `le` tag for each
bound that is greater than or equal to the value, plus a final `le=inf` bucket
so the histogram is cumulative per the statsd `le` convention:

```rust
metrics.histogram("codex.request_latency", 83, &buckets, &[("route", "chat")])?;
```

`counter`, `histogram`, and `send` enqueue metrics for the background worker.
Call `shutdown` to flush queued metrics on exit.

## Timing

Measure a closure and emit a histogram sample for the elapsed time in milliseconds:

```rust
let result = metrics.time("codex.request_latency", &buckets, &[("route", "chat")], || {
    "ok"
})?;
```

If the closure already returns `codex_metrics::Result<T>`, use `time_result` to
avoid nested results:

```rust
let result = metrics.time_result("codex.request_latency", &buckets, &[("route", "chat")], || {
    Ok("ok")
})?;
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

Batching reduces network requests and ensure metrics have the same timestamp.

```rust
let mut batch = metrics.batch();
batch.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
batch.histogram("codex.tool_latency", 140, &buckets, &[("tool", "shell")])?;
metrics.send(batch)?;
```

## Shutdown and queue capacity

The client uses a bounded queue (default capacity 1024). Enqueueing returns a
`MetricsError::QueueFull` error if the queue is full or `MetricsError::WorkerUnavailable`
if the worker is no longer running.

`shutdown` waits up to 500ms for the worker to stop.

Uploads are best-effort; if the worker encounters a send error, the metric is
dropped (if in `alpha`, or debug mode, the worker will panic on errors).

`MetricsClient` also attempts a best-effort shutdown on drop using the default
timeout, so explicit calls to `shutdown` are optional.

## Validation rules

Metric names:

- Must be non-empty.
- Allowed characters: ASCII letters/digits plus `.`, `_`, `-`.

Tag keys and values:

- Must be non-empty.
- Allowed characters: ASCII letters/digits plus `.`, `_`, `-`, `/`.

## Error handling

All APIs return `codex_metrics::Result<T>` with a `MetricsError` variant on
failure. Errors cover invalid configuration, validation failures, and HTTP or
serialization failures.
