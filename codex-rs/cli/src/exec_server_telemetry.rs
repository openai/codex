use std::future::Future;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const DEFAULT_ANALYTICS_ENABLED: bool = false;
const DEFAULT_LOG_FILTER: &str = "error,opentelemetry_sdk=off,opentelemetry_otlp=off";
const OTEL_SERVICE_NAME: &str = "codex-exec-server";

pub(crate) fn init(
    config: Option<&codex_core::config::Config>,
) -> (impl Send + Sync, codex_exec_server::ExecServerTelemetry) {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_env_filter());
    let otel = match config {
        Some(config) => codex_core::otel_init::build_provider(
            config,
            env!("CARGO_PKG_VERSION"),
            Some(OTEL_SERVICE_NAME),
            DEFAULT_ANALYTICS_ENABLED,
        )
        .unwrap_or_else(|error| {
            eprintln!("Could not create otel exporter: {error}");
            None
        }),
        None => None,
    };
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
    (otel, telemetry)
}

pub(crate) async fn run_until_shutdown<F, E>(run: F) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
{
    let shutdown_signal = match shutdown_signal() {
        Ok(signal) => Some(signal),
        Err(error) => {
            eprintln!("Could not listen for exec-server shutdown signal: {error}");
            None
        }
    };
    tokio::pin!(run);

    if let Some(shutdown_signal) = shutdown_signal {
        tokio::select! {
            result = &mut run => result,
            signal = wait_for_shutdown_signal(shutdown_signal) => {
                match signal {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        eprintln!("Could not listen for exec-server shutdown signal: {error}");
                        run.await
                    }
                }
            }
        }
    } else {
        run.await
    }
}

#[cfg(unix)]
struct ShutdownSignal {
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
fn shutdown_signal() -> std::io::Result<ShutdownSignal> {
    Ok(ShutdownSignal {
        terminate: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
    })
}

#[cfg(unix)]
async fn wait_for_shutdown_signal(mut shutdown_signal: ShutdownSignal) -> std::io::Result<()> {
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        _ = shutdown_signal.terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
struct ShutdownSignal;

#[cfg(not(unix))]
fn shutdown_signal() -> std::io::Result<ShutdownSignal> {
    Ok(ShutdownSignal)
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal(_: ShutdownSignal) -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}

fn stderr_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(DEFAULT_LOG_FILTER))
        .unwrap_or_else(|_| EnvFilter::new("error"))
}
