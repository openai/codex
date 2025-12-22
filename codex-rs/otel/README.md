# codex-otel

`codex-otel` is the OpenTelemetry integration crate for Codex. It provides:

- Trace/log exporters and tracing subscriber layers (`codex_otel::traces::otel_provider`).
- A structured event helper (`codex_otel::OtelManager`).
- A Statsig `log_event` metrics client (`codex_otel::metrics`).
- A metrics facade on `OtelManager` so tracing + metrics share metadata.

## Tracing and logs

Create an OTEL provider from `OtelSettings`, then attach its layers to your
`tracing_subscriber` registry:

```rust
use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::config::OtelSettings;
use codex_otel::traces::otel_provider::OtelProvider;
use tracing_subscriber::prelude::*;

let settings = OtelSettings {
    environment: "dev".to_string(),
    service_name: "codex-cli".to_string(),
    service_version: env!("CARGO_PKG_VERSION").to_string(),
    codex_home: std::path::PathBuf::from("/tmp"),
    exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".to_string(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    trace_exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".to_string(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    metrics: None,
};

if let Some(provider) = OtelProvider::from(&settings)? {
    let registry = tracing_subscriber::registry()
        .with(provider.logger_layer())
        .with(provider.tracing_layer());
    registry.init();
}
```

## OtelManager (events)

`OtelManager` adds consistent metadata to tracing events and helps record
Codex-specific events.

```rust
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
);

manager.user_prompt(&prompt_items);
```

## Metrics (Statsig HTTP or in-memory)

Statsig example:

```rust
let metrics = MetricsClient::new(MetricsConfig::default())?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

In-memory (tests):

```rust
let exporter = InMemoryMetricExporter::default();
let metrics = MetricsClient::new(MetricsConfig::in_memory(exporter.clone()))?;
metrics.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
metrics.shutdown()?; // flushes in-memory exporter
```

## Shutdown

- `OtelProvider::shutdown()` stops the OTEL exporter.
- `OtelManager::shutdown_metrics()` flushes and stops the metrics worker.

Both are optional because drop performs best-effort shutdown, but calling them
explicitly gives deterministic flushing (or a shutdown error if flushing does
not complete in time).
