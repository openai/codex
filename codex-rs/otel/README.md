# codex-otel

`codex-otel` is the OpenTelemetry integration crate for Codex. It provides:

- Trace/log/metrics exporters and tracing subscriber layers (`codex_otel::otel_provider`).
- A structured event helper (`codex_otel::OtelManager`).
- OpenTelemetry metrics support via OTLP exporters (`codex_otel::metrics`).
- A metrics facade on `OtelManager` so tracing + metrics share metadata.

## Tracing and logs

Create an OTEL provider from `OtelSettings`. The provider also configures
metrics (when enabled), then attach its layers to your `tracing_subscriber`
registry:

```rust
use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::config::OtelSettings;
use codex_otel::otel_provider::OtelProvider;
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
    metrics_exporter: OtelExporter::None,
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

## Metrics (OTLP or in-memory)

Modes:

- OTLP: exports metrics via the OpenTelemetry OTLP exporter (HTTP or gRPC).
- In-memory: records via `opentelemetry_sdk::metrics::InMemoryMetricExporter` for tests/assertions; call `shutdown()` to flush.

`codex-otel` also provides `OtelExporter::Statsig`, a shorthand for exporting OTLP/HTTP JSON metrics
to Statsig using Codex-internal defaults.

Statsig ingestion (OTLP/HTTP JSON) example:

```rust
use codex_otel::config::{OtelExporter, OtelHttpProtocol};

let metrics = MetricsClient::new(MetricsConfig::otlp(
    "dev",
    "codex-cli",
    env!("CARGO_PKG_VERSION"),
    OtelExporter::OtlpHttp {
        endpoint: "https://api.statsig.com/otlp".to_string(),
        headers: std::collections::HashMap::from([(
            "statsig-api-key".to_string(),
            std::env::var("STATSIG_SERVER_SDK_SECRET")?,
        )]),
        protocol: OtelHttpProtocol::Json,
        tls: None,
    },
))?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
metrics.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

In-memory (tests):

```rust
let exporter = InMemoryMetricExporter::default();
let metrics = MetricsClient::new(MetricsConfig::in_memory(
    "test",
    "codex-cli",
    env!("CARGO_PKG_VERSION"),
    exporter.clone(),
))?;
metrics.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
metrics.shutdown()?; // flushes in-memory exporter
```

## Shutdown

- `OtelProvider::shutdown()` stops the OTEL exporter.
- `OtelManager::shutdown_metrics()` flushes and shuts down the metrics provider.

Both are optional because drop performs best-effort shutdown, but calling them
explicitly gives deterministic flushing (or a shutdown error if flushing does
not complete in time).
