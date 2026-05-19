use std::collections::HashMap;
use std::sync::Arc;

use codex_config::NoopThreadConfigLoader;
use codex_config::ThreadConfigContext;
use codex_config::ThreadConfigLoader;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_json_to_toml::json_to_toml;

/// Read-only app-owned access to the prepared config snapshot used at runtime.
pub trait ConfigProvider: Send + Sync {
    /// Returns the effective config snapshot that new runtime consumers should use.
    fn current(&self) -> PreparedConfig;
}

/// Effective config snapshot prepared by app/bootstrap before core consumes it.
#[derive(Clone)]
pub struct PreparedConfig {
    config: Arc<Config>,
    reload_thread_config: bool,
    thread_config_loader: Arc<dyn ThreadConfigLoader>,
}

impl PreparedConfig {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            reload_thread_config: false,
            thread_config_loader: Arc::new(NoopThreadConfigLoader),
        }
    }

    pub fn reloadable(config: Arc<Config>) -> Self {
        Self {
            config,
            reload_thread_config: true,
            thread_config_loader: Arc::new(NoopThreadConfigLoader),
        }
    }

    pub fn with_thread_config_loader(
        mut self,
        thread_config_loader: Arc<dyn ThreadConfigLoader>,
    ) -> Self {
        self.thread_config_loader = thread_config_loader;
        self
    }

    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    pub(crate) fn reloads_thread_config(&self) -> bool {
        self.reload_thread_config
    }

    pub(crate) async fn derive_thread_config(
        &self,
        request_overrides: Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: ConfigOverrides,
    ) -> std::io::Result<Config> {
        let session_overrides = request_overrides
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (key, json_to_toml(value)))
            .collect();
        let cwd = match typesafe_overrides.cwd.as_deref() {
            Some(cwd) => AbsolutePathBuf::relative_to_current_dir(cwd)?,
            None => self.config.cwd.clone(),
        };
        let thread_config_layers = self
            .thread_config_loader
            .load_config_layers(ThreadConfigContext {
                thread_id: None,
                cwd: Some(cwd),
            })
            .await
            .map_err(std::io::Error::other)?;
        self.config
            .derive_thread_config(session_overrides, thread_config_layers, typesafe_overrides)
            .await
    }
}

/// Read-only provider for callers that already prepared a fixed config snapshot.
pub struct StaticConfigProvider {
    current: PreparedConfig,
}

impl StaticConfigProvider {
    pub fn new(current: PreparedConfig) -> Self {
        Self { current }
    }
}

impl ConfigProvider for StaticConfigProvider {
    fn current(&self) -> PreparedConfig {
        self.current.clone()
    }
}
