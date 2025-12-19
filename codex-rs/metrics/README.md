# codex-metrics

Send lightweight counters and histogram buckets to Sentry via the statsd envelope item.

## Overview

- Non-blocking client that enqueues metrics to a background worker.
- Counters and histograms only (histograms are encoded as bucketed counters).
- Tag validation and metric name validation are enforced before send.

## Quick start

```rust
use codex_metrics::HistogramBuckets;
use codex_metrics::MetricsClient;
use codex_metrics::MetricsConfig;
use codex_metrics::Result;

fn main() -> Result<()> {
    let metrics = MetricsClient::new(
        MetricsConfig::new("https://public@example.ingest.us.sentry.io/123456")
            .with_tag("service", "codex-cli")?
            .with_tag("env", "dev")?,
    )?;

    let buckets = HistogramBuckets::from_values(&[25, 50, 100, 250, 500, 1000])?;

    metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
    metrics.histogram("codex.request_latency", 83, &buckets, &[("route", "chat")])?;

    Ok(())
}
```

Buckets are integer upper bounds; pick your own unit (ms, bytes, tokens, etc.).

You can also use the default placeholder DSN:

```rust
let metrics = MetricsClient::new(MetricsConfig::default())?;
```

## Configuration

`MetricsConfig` lets you specify:

- `MetricsConfig::new(dsn)` to set the Sentry DSN.
- `with_tag(key, value)` to add default tags.
- `with_timeout(duration)` to override the HTTP timeout (default 10s).
- `with_user_agent(agent)` to override the user agent.
Use `MetricsClient::with_capacity(config, capacity)` to override the default
queue capacity.

## Buckets

`HistogramBuckets` supports two constructors:

- `from_values(&[...])` for explicit upper bounds.
- `from_range(from, to, step)` to build linear buckets.
- `from_exponential(from, to, factor)` to build exponential buckets.

`from_range` requires `step > 0` and `from <= to`. The upper bound is always included.
`from_exponential` requires `from > 0`, `from <= to`, and a finite `factor > 1`. The upper bound is always included.

```rust
let buckets = HistogramBuckets::from_range(25, 100, 25)?;
let exp_buckets = HistogramBuckets::from_exponential(10, 1000, 2.0)?;
```

## Sending metrics

Counters send a single statsd counter increment with tags:

```rust
metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
```

Histograms are translated into bucket counters by adding an `le` tag for each
bound that is greater than or equal to the value (or `inf` if none match):

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

Batching reduces network requests. Build a batch and send it once:

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

`shutdown` waits up to 500ms for the worker to stop. Use `shutdown_with_timeout`
to override the timeout.

Uploads are best-effort; if the worker encounters a send error, the metric is
dropped.

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
