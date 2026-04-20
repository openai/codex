//! App-server OpenTelemetry reloading for configuration resolved after startup.
//!
//! The app server installs its tracing subscriber once, but `thread/start` can
//! later load project-scoped config from the requested cwd. This module keeps
//! the installed log layer stable while swapping the underlying OTel provider
//! when that effective thread config changes.

use codex_config::types::OtelConfig;
use codex_core::config::Config;
use codex_otel::OtelLoggerLayer;
use codex_otel::OtelProvider;
use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
struct OtelProviderKey {
    otel: OtelConfig,
    analytics_enabled: Option<bool>,
    default_analytics_enabled: bool,
}

struct OtelReloadState {
    key: Option<OtelProviderKey>,
    provider: Option<OtelProvider>,
}

#[derive(Clone)]
pub(crate) struct OtelReloader {
    logger_layer: OtelLoggerLayer,
    state: Arc<Mutex<OtelReloadState>>,
    default_analytics_enabled: bool,
}

impl OtelReloader {
    pub(crate) fn new(
        initial_config: &Config,
        provider: Option<OtelProvider>,
        default_analytics_enabled: bool,
    ) -> (OtelLoggerLayer, OtelReloader) {
        let logger_layer = OtelLoggerLayer::from_provider(provider.as_ref());
        (
            logger_layer.clone(),
            OtelReloader {
                logger_layer,
                state: Arc::new(Mutex::new(OtelReloadState {
                    key: Some(provider_key(initial_config, default_analytics_enabled)),
                    provider,
                })),
                default_analytics_enabled,
            },
        )
    }

    pub(crate) fn reload_from_config(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        let next_key = provider_key(config, self.default_analytics_enabled);
        {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.key.as_ref() == Some(&next_key) {
                return Ok(());
            }
        }

        let next_provider = codex_core::otel_init::build_provider(
            config,
            env!("CARGO_PKG_VERSION"),
            Some("codex-app-server"),
            self.default_analytics_enabled,
        )?;
        self.logger_layer.replace_provider(next_provider.as_ref());

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.key = Some(next_key);
        state.provider = next_provider;
        Ok(())
    }

    pub(crate) fn shutdown(&self) {
        self.logger_layer.replace_provider(/*provider*/ None);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.provider.take();
    }
}

fn provider_key(config: &Config, default_analytics_enabled: bool) -> OtelProviderKey {
    OtelProviderKey {
        otel: config.otel.clone(),
        analytics_enabled: config.analytics_enabled,
        default_analytics_enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::types::OtelExporterKind;
    use codex_config::types::OtelHttpProtocol;
    use codex_core::config::ConfigBuilder;
    use std::collections::HashMap;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::timeout;
    use tracing_subscriber::prelude::*;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test(flavor = "multi_thread")]
    async fn reload_from_config_updates_log_exporter() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/logs"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let codex_home = TempDir::new()?;
        let initial_config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        let (layer, reloader) = OtelReloader::new(
            &initial_config,
            /*provider*/ None,
            /*default_analytics_enabled*/ false,
        );
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut project_config = initial_config.clone();
        project_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/v1/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };

        reloader.reload_from_config(&project_config)?;
        tracing::event!(
            target: "codex_otel.log_only",
            tracing::Level::INFO,
            event.name = "codex.reload_test",
        );
        reloader.shutdown();

        let body = wait_for_otel_logs_payload(&server).await?;
        let body = String::from_utf8(body)?;
        assert!(
            body.contains("codex.reload_test"),
            "expected reloaded OTEL logs to include test event; body prefix: {}",
            body.chars().take(2000).collect::<String>()
        );
        Ok(())
    }

    async fn wait_for_otel_logs_payload(server: &MockServer) -> Result<Vec<u8>, Box<dyn Error>> {
        let body = timeout(Duration::from_secs(10), async {
            loop {
                let Some(requests) = server.received_requests().await else {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    continue;
                };
                if let Some(request) = requests
                    .iter()
                    .find(|request| request.method == "POST" && request.url.path() == "/v1/logs")
                {
                    return Ok::<Vec<u8>, Box<dyn Error>>(request.body.clone());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await??;
        Ok(body)
    }
}
