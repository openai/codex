use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_ANALYTICS_ENABLED: bool = false;
const DEFAULT_LOG_FILTER: &str = "error,opentelemetry_sdk=off,opentelemetry_otlp=off";
const OTEL_SERVICE_NAME: &str = "codex-exec-server";

pub(crate) fn init(
    config: Option<&codex_core::config::Config>,
) -> Result<(impl Send + Sync, codex_exec_server::ExecServerTelemetry), Box<dyn std::error::Error>>
{
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
    }?;
    let provider = otel.as_ref();
    codex_core::otel_init::record_process_start(provider, OTEL_SERVICE_NAME);

    let otel_logger_layer = provider.and_then(|otel| otel.logger_layer());
    let otel_tracing_layer = provider.and_then(|otel| otel.tracing_layer());
    let telemetry = provider
        .and_then(|otel| otel.metrics())
        .cloned()
        .map(codex_exec_server::ExecServerTelemetry::new)
        .unwrap_or_default();
    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_tracing_layer)
        .with(otel_logger_layer)
        .try_init();
    tracing::callsite::rebuild_interest_cache();
    Ok((otel, telemetry))
}

pub(crate) fn init_or_default(
    config: Option<&codex_core::config::Config>,
) -> (impl Send + Sync, codex_exec_server::ExecServerTelemetry) {
    match init(config) {
        Ok(initialized) => initialized,
        Err(error) => {
            eprintln!("Could not create otel exporter: {error}");
            match init(/*config*/ None) {
                Ok(initialized) => initialized,
                Err(error) => panic!("failed to initialize exec-server logging: {error}"),
            }
        }
    }
}

fn stderr_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(DEFAULT_LOG_FILTER))
        .unwrap_or_else(|_| EnvFilter::new("error"))
}
