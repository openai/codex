use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_ANALYTICS_ENABLED: bool = false;
const DEFAULT_LOG_FILTER: &str = "error,opentelemetry_sdk=off,opentelemetry_otlp=off";
const OTEL_SERVICE_NAME: &str = "codex-exec-server";

pub(crate) fn init(config: Option<&codex_core::config::Config>) -> impl Send + Sync {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_env_filter());
    let otel = config.and_then(|config| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codex_core::otel_init::build_provider(
                config,
                env!("CARGO_PKG_VERSION"),
                Some(OTEL_SERVICE_NAME),
                DEFAULT_ANALYTICS_ENABLED,
            )
        })) {
            Ok(Ok(otel)) => otel,
            Ok(Err(err)) => {
                eprintln!("Could not create otel exporter: {err}");
                None
            }
            Err(_) => {
                eprintln!("Could not create otel exporter: panicked during initialization");
                None
            }
        }
    });
    codex_core::otel_init::record_process_start(otel.as_ref(), OTEL_SERVICE_NAME);

    let otel_logger_layer = otel.as_ref().and_then(|otel| otel.logger_layer());
    let otel_tracing_layer = otel.as_ref().and_then(|otel| otel.tracing_layer());
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
