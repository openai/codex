//! Host-assembled plugin sources; discovery results belong to the active thread, not this registry.

use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginProvider;
use std::sync::Arc;

/// Cloud discovery is optional and never replaces executor plugin support.
pub struct PluginProviders {
    // CCA owns and supplies the cloud provider implementation.
    pub(crate) cloud: Option<Arc<dyn PluginProvider>>,
    // Keep concrete access to resolve_bound so loaders retain the exact executor filesystem.
    pub(crate) executor: Arc<ExecutorPluginProvider>,
}

impl PluginProviders {
    pub fn new(executor: Arc<ExecutorPluginProvider>) -> Self {
        Self {
            cloud: None,
            executor,
        }
    }

    pub fn with_cloud_provider(mut self, provider: Arc<dyn PluginProvider>) -> Self {
        self.cloud = Some(provider);
        self
    }
}
