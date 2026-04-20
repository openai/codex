//! App-server OpenTelemetry reloading for configuration resolved after startup.
//!
//! The app server installs its tracing subscriber once, before `thread/start`
//! can load project-scoped config from the requested cwd. This module keeps the
//! installed OTel layers stable while initializing their provider from the first
//! effective thread config.

use codex_config::types::OtelConfig;
use codex_core::config::Config;
use codex_otel::OtelLoggerLayer;
use codex_otel::OtelProvider;
use codex_otel::OtelTraceLayer;
use codex_otel::OtelTraceLayerHandle;
use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

#[derive(Clone, Debug, PartialEq)]
struct OtelProviderKey {
    otel: OtelConfig,
    analytics_enabled: Option<bool>,
    default_analytics_enabled: bool,
}

struct OtelReloadState {
    key: Option<OtelProviderKey>,
    provider: Option<OtelProvider>,
    retired_providers: Vec<OtelProvider>,
    initialized_from_thread_config: bool,
}

#[derive(Clone)]
pub(crate) struct OtelReloader {
    logger_layer: OtelLoggerLayer,
    trace_layer: OtelTraceLayerHandle,
    state: Arc<Mutex<OtelReloadState>>,
    default_analytics_enabled: bool,
}

impl OtelReloader {
    pub(crate) fn new<S>(
        initial_config: &Config,
        provider: Option<OtelProvider>,
        default_analytics_enabled: bool,
    ) -> (OtelLoggerLayer, OtelTraceLayer<S>, OtelReloader)
    where
        S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
    {
        let logger_layer = OtelLoggerLayer::from_provider(provider.as_ref());
        let (trace_layer, trace_layer_handle) = OtelTraceLayer::from_provider(provider.as_ref());
        (
            logger_layer.clone(),
            trace_layer,
            OtelReloader {
                logger_layer,
                trace_layer: trace_layer_handle,
                state: Arc::new(Mutex::new(OtelReloadState {
                    key: Some(provider_key(initial_config, default_analytics_enabled)),
                    provider,
                    retired_providers: Vec::new(),
                    initialized_from_thread_config: false,
                })),
                default_analytics_enabled,
            },
        )
    }

    pub(crate) fn reload_from_config(
        &self,
        config: &Config,
    ) -> Result<OtelReloadCommit, Box<dyn Error>> {
        let next_key = provider_key(config, self.default_analytics_enabled);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.key.as_ref() == Some(&next_key) {
            return Ok(OtelReloadCommit::new(
                Arc::clone(&self.state),
                next_key,
                state.key.clone(),
                /*old_provider*/ None,
                self.logger_layer.clone(),
                self.trace_layer.clone(),
                /*applied_reload*/ false,
            ));
        }
        if state.initialized_from_thread_config {
            return Err("app-server OTel config is already initialized from a different effective thread config; restart the app server to use a different project OTel config".into());
        }

        let next_provider = codex_core::otel_init::build_provider(
            config,
            env!("CARGO_PKG_VERSION"),
            Some("codex-app-server"),
            self.default_analytics_enabled,
        )?;
        self.logger_layer.replace_provider(next_provider.as_ref());
        self.trace_layer.replace_provider(next_provider.as_ref());
        let previous_key = state.key.clone();
        state.key = Some(next_key.clone());
        let old_provider = std::mem::replace(&mut state.provider, next_provider);
        Ok(OtelReloadCommit::new(
            Arc::clone(&self.state),
            next_key,
            previous_key,
            old_provider,
            self.logger_layer.clone(),
            self.trace_layer.clone(),
            /*applied_reload*/ true,
        ))
    }

    pub(crate) fn shutdown(&self) {
        self.logger_layer.replace_provider(/*provider*/ None);
        self.trace_layer.shutdown();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.provider.take();
        state.retired_providers.clear();
    }
}

pub(crate) struct OtelReloadCommit {
    state: Arc<Mutex<OtelReloadState>>,
    key: OtelProviderKey,
    previous_key: Option<OtelProviderKey>,
    old_provider: Option<OtelProvider>,
    logger_layer: OtelLoggerLayer,
    trace_layer: OtelTraceLayerHandle,
    applied_reload: bool,
    committed: bool,
}

impl OtelReloadCommit {
    fn new(
        state: Arc<Mutex<OtelReloadState>>,
        key: OtelProviderKey,
        previous_key: Option<OtelProviderKey>,
        old_provider: Option<OtelProvider>,
        logger_layer: OtelLoggerLayer,
        trace_layer: OtelTraceLayerHandle,
        applied_reload: bool,
    ) -> Self {
        Self {
            state,
            key,
            previous_key,
            old_provider,
            logger_layer,
            trace_layer,
            applied_reload,
            committed: false,
        }
    }

    pub(crate) fn commit(mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.key.as_ref() == Some(&self.key) {
            if let Some(old_provider) = self.old_provider.take() {
                state.retired_providers.push(old_provider);
            }
            state.initialized_from_thread_config = true;
            self.committed = true;
        }
    }
}

impl Drop for OtelReloadCommit {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        if !self.applied_reload {
            return;
        }

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.initialized_from_thread_config && state.key.as_ref() == Some(&self.key) {
            let _abandoned_provider =
                std::mem::replace(&mut state.provider, self.old_provider.take());
            state.key = self.previous_key.clone();
            self.logger_layer.replace_provider(state.provider.as_ref());
            self.trace_layer.replace_provider(state.provider.as_ref());
        } else if let Some(old_provider) = self.old_provider.take() {
            state.retired_providers.push(old_provider);
        }
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
        let (logger_layer, trace_layer, reloader) = OtelReloader::new(
            &initial_config,
            /*provider*/ None,
            /*default_analytics_enabled*/ false,
        );
        let subscriber = tracing_subscriber::registry()
            .with(logger_layer)
            .with(trace_layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let mut project_config = initial_config.clone();
        project_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/v1/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };

        reloader.reload_from_config(&project_config)?.commit();
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

    #[tokio::test(flavor = "multi_thread")]
    async fn reload_from_config_rejects_different_second_config() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start().await;
        let codex_home = TempDir::new()?;
        let initial_config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        let (_logger_layer, _trace_layer, reloader): (
            _,
            OtelTraceLayer<tracing_subscriber::Registry>,
            _,
        ) = OtelReloader::new(
            &initial_config,
            /*provider*/ None,
            /*default_analytics_enabled*/ false,
        );

        let mut first_config = initial_config.clone();
        first_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/v1/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };
        reloader.reload_from_config(&first_config)?.commit();

        let mut second_config = initial_config;
        second_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/other/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };
        let err = match reloader.reload_from_config(&second_config) {
            Ok(_) => panic!("different effective OTel config should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("already initialized from a different effective thread config"),
            "unexpected error: {err}"
        );

        reloader.shutdown();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reload_from_config_does_not_pin_until_committed() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start().await;
        let codex_home = TempDir::new()?;
        let initial_config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        let (_logger_layer, _trace_layer, reloader): (
            _,
            OtelTraceLayer<tracing_subscriber::Registry>,
            _,
        ) = OtelReloader::new(
            &initial_config,
            /*provider*/ None,
            /*default_analytics_enabled*/ false,
        );

        let mut first_config = initial_config.clone();
        first_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/v1/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };
        let first_reload = reloader.reload_from_config(&first_config)?;
        drop(first_reload);
        {
            let state = reloader
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(state.provider.is_none());
            assert!(state.retired_providers.is_empty());
        }

        let mut second_config = initial_config;
        second_config.otel.exporter = OtelExporterKind::OtlpHttp {
            endpoint: format!("{}/other/logs", server.uri()),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        };
        reloader.reload_from_config(&second_config)?.commit();

        let err = match reloader.reload_from_config(&first_config) {
            Ok(_) => panic!("different effective OTel config should be rejected after commit"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("already initialized from a different effective thread config"),
            "unexpected error: {err}"
        );

        reloader.shutdown();
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
