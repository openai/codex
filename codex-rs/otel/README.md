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

## Metrics (Statsig HTTP)

The metrics client sends counters and histograms to Statsig via the `log_event`
endpoint. Use placeholders for the Statsig endpoint and API key header until
you have real values:

```rust
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;

let metrics = MetricsClient::new(
    MetricsConfig::new("<statsig-api-key>")
        .with_endpoint("<statsig-log-event-endpoint>")
        .with_api_key_header("<statsig-api-key-header>"),
)?;

metrics.counter("codex.session_started", 1, &[("source", "tui")])?;
```

## Metrics via OtelManager

Attach metrics once in `OtelSettings.metrics` and reuse them from
`OtelManager`:

```rust
use codex_otel::config::{OtelExporter, OtelHttpProtocol, OtelSettings};
use codex_otel::metrics::MetricsConfig;
use codex_otel::OtelManager;
use codex_otel::traces::otel_provider::OtelProvider;
use tracing_subscriber::prelude::*;

let settings = OtelSettings {
    environment: "dev".into(),
    service_name: "codex-cli".into(),
    service_version: env!("CARGO_PKG_VERSION").into(),
    codex_home: std::path::PathBuf::from("/tmp"),
    exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".into(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    trace_exporter: OtelExporter::OtlpHttp {
        endpoint: "https://otlp.example.com".into(),
        headers: std::collections::HashMap::new(),
        protocol: OtelHttpProtocol::Binary,
        tls: None,
    },
    metrics: Some(
        MetricsConfig::new("<statsig-api-key>")
            .with_endpoint("<statsig-log-event-endpoint>")
            .with_api_key_header("<statsig-api-key-header>"),
    ),
};

let provider = OtelProvider::from(&settings)?;
if let Some(p) = &provider {
    tracing_subscriber::registry()
        .with(p.logger_layer())
        .with(p.tracing_layer())
        .init();
}

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
let manager = provider
    .as_ref()
    .map(|p| manager.with_provider_metrics(p))
    .unwrap_or(manager);

manager.counter("codex.session_started", 1, &[("source", "tui")])?;
manager.histogram("codex.request_latency", 83, &[("route", "chat")])?;
```

By default, `OtelManager` adds metadata tags to metrics: `auth_mode`, `model`,
`slug`, `terminal.type`, and `app.version`. Use
`with_metrics_without_metadata_tags` to disable these tags.

## Shutdown

- `OtelProvider::shutdown()` stops the OTEL exporter.
- `OtelManager::shutdown_metrics()` flushes and stops the metrics worker.

Both are optional because drop performs best-effort shutdown, but calling them
explicitly gives deterministic flushing.
