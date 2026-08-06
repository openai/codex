use crate::OTEL_SERVICE_NAME;
use crate::config_manager::ConfigManager;
use codex_login::AuthManager;
use codex_otel::OtelProvider;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::Subscriber;
use tracing::info;
use tracing::warn;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;

type OtelExportLayer<S> = Option<Box<dyn Layer<S> + Send + Sync + 'static>>;
type OtelReloadLayers<S> = (
    Vec<Box<dyn Layer<S> + Send + Sync + 'static>>,
    reload::Handle<OtelExportLayer<S>, S>,
);

pub(crate) fn layers<S>(provider: Option<&OtelProvider>) -> OtelReloadLayers<S>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    let logger_export_layer: OtelExportLayer<S> = provider
        .and_then(OtelProvider::logger_export_layer)
        .map(Layer::boxed);
    let (logger_layer, logger_handle) = reload::Layer::new(logger_export_layer);

    let mut layers: Vec<Box<dyn Layer<S> + Send + Sync + 'static>> = vec![
        logger_layer
            .with_filter(tracing_subscriber::filter::filter_fn(
                OtelProvider::log_export_filter,
            ))
            .boxed(),
    ];
    if provider.is_some_and(|provider| provider.tracer.is_some()) {
        layers.push(OtelProvider::reloadable_tracing_layer(OTEL_SERVICE_NAME).boxed());
    }
    (layers, logger_handle)
}

pub(crate) fn spawn<S>(
    mut provider: Option<OtelProvider>,
    logger_reload_handle: reload::Handle<OtelExportLayer<S>, S>,
    config_manager: ConfigManager,
    auth_manager: Arc<AuthManager>,
    default_analytics_enabled: bool,
    shutdown_token: CancellationToken,
) -> JoinHandle<()>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    let mut auth_changes = auth_manager.auth_change_receiver();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => break,
                changed = auth_changes.changed() => {
                    if changed.is_err() {
                        break;
                    }

                    // Account handlers install the new cloud loader after publishing auth changes.
                    tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await;

                    let config = match config_manager.load_latest_config(/*fallback_cwd*/ None).await {
                        Ok(config) => config,
                        Err(error) => {
                            warn!(%error, "failed to reload telemetry config after account change");
                            continue;
                        }
                    };
                    let next_provider = match codex_core::otel_init::build_provider(
                        &config,
                        env!("CARGO_PKG_VERSION"),
                        Some(OTEL_SERVICE_NAME),
                        default_analytics_enabled,
                    ) {
                        Ok(provider) => provider,
                        Err(error) => {
                            warn!(%error, "failed to rebuild telemetry exporters after account change");
                            continue;
                        }
                    };
                    if let Err(error) = logger_reload_handle.reload(
                        next_provider
                            .as_ref()
                            .and_then(OtelProvider::logger_export_layer)
                            .map(Layer::boxed),
                    ) {
                        warn!(%error, "failed to install telemetry exporters after account change");
                        continue;
                    }
                    if let Some(previous_provider) = std::mem::replace(&mut provider, next_provider) {
                        drop(tokio::task::spawn_blocking(move || previous_provider.shutdown()));
                    }
                    info!(
                        event.name = "codex.app_server.otel_reloaded",
                        "reloaded telemetry exporters after account change"
                    );
                }
            }
        }

        if let Some(provider) = provider {
            let _ = tokio::task::spawn_blocking(move || provider.shutdown()).await;
        }
    })
}
