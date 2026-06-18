use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_ANALYTICS_ENABLED: bool = false;
const DEFAULT_LOG_FILTER: &str = "error,opentelemetry_sdk=off,opentelemetry_otlp=off";
const OTEL_SERVICE_NAME: &str = "codex-exec-server";

pub(crate) fn init(
    config: Option<&codex_core::config::Config>,
) -> Result<impl Send + Sync, Box<dyn std::error::Error>> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_env_filter());
    let otel = match config {
        Some(config) => codex_core::otel_init::build_provider(
            config,
            env!("CARGO_PKG_VERSION"),
            Some(OTEL_SERVICE_NAME),
            DEFAULT_ANALYTICS_ENABLED,
        ),
        None => Ok(None),
    };
    let provider = otel.as_ref().ok().and_then(Option::as_ref);
    codex_core::otel_init::record_process_start(provider, OTEL_SERVICE_NAME);

    let otel_logger_layer = provider.and_then(|otel| otel.logger_layer());
    let otel_tracing_layer = provider.and_then(|otel| otel.tracing_layer());
    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_tracing_layer)
        .with(otel_logger_layer)
        .try_init();
    tracing::callsite::rebuild_interest_cache();
    otel
}

fn stderr_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(DEFAULT_LOG_FILTER))
        .unwrap_or_else(|_| EnvFilter::new("error"))
}
