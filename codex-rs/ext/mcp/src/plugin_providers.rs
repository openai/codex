//! Host-assembled plugin sources; discovery results belong to the active thread, not this registry.

use codex_core::config::Config;
use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginProvider;
use codex_features::Feature;
use std::sync::Arc;

type CloudEnabled = dyn Fn(&Config) -> bool + Send + Sync;

/// Cloud discovery is optional and never replaces executor plugin support.
pub struct PluginProviders {
    // CCA owns and supplies the cloud provider implementation.
    pub(crate) cloud: Option<Arc<dyn PluginProvider>>,
    // Keep concrete access to resolve_bound so loaders retain the exact executor filesystem.
    pub(crate) executor: Arc<ExecutorPluginProvider>,
    cloud_enabled: Option<Arc<CloudEnabled>>,
}

impl PluginProviders {
    pub fn new(executor: Arc<ExecutorPluginProvider>) -> Self {
        Self {
            cloud: None,
            executor,
            cloud_enabled: None,
        }
    }

    pub fn with_cloud_provider(mut self, provider: Arc<dyn PluginProvider>) -> Self {
        self.cloud = Some(provider);
        self
    }

    /// Restricts cloud discovery using effective host configuration, independently of executor
    /// plugins. Configured sources with unavailable connections must remain enabled for recovery.
    pub fn with_cloud_enabled(
        mut self,
        enabled: impl Fn(&Config) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.cloud_enabled = Some(Arc::new(enabled));
        self
    }

    pub(crate) fn cloud_enabled(&self, config: &Config) -> bool {
        self.cloud.is_some()
            && config.features.enabled(Feature::Plugins)
            && self
                .cloud_enabled
                .as_ref()
                .is_none_or(|enabled| enabled(config))
    }
}
