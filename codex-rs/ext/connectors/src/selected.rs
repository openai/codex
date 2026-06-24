use codex_connectors::ConnectorSnapshot;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::ExecutorPluginProvider;
use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::sync::Arc;

use crate::ExecutorPluginConnectorProvider;

/// Resolves connector declarations from thread-selected executor plugins.
#[derive(Clone, Debug)]
pub struct SelectedExecutorConnectorProvider {
    plugin_provider: ExecutorPluginProvider,
    connector_provider: ExecutorPluginConnectorProvider,
}

impl SelectedExecutorConnectorProvider {
    /// Creates a provider backed by the active execution environments.
    pub fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            plugin_provider: ExecutorPluginProvider::new(environment_manager),
            connector_provider: ExecutorPluginConnectorProvider,
        }
    }

    /// Resolves one immutable connector snapshot in selected-root order.
    pub async fn snapshot_for_roots(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> ConnectorSnapshot {
        let mut sources = Vec::new();

        for selected_root in selected_roots {
            let plugin = match self.plugin_provider.resolve_bound(selected_root).await {
                Ok(Some(plugin)) => plugin,
                Ok(None) => continue,
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to resolve selected executor plugin for connector discovery"
                    );
                    continue;
                }
            };
            match self.connector_provider.load(&plugin).await {
                Ok(declarations) => sources.push(PluginConnectorSource::new(
                    plugin.plugin().selected_root_id(),
                    plugin.plugin().manifest().display_name(),
                    declarations,
                )),
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to load selected executor plugin connectors"
                    );
                }
            }
        }

        ConnectorSnapshot::from_plugin_sources(sources)
    }
}
