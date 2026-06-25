use codex_connectors::ConnectorSnapshot;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::SelectedCapabilityBindings;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ThreadExtensionInitContributor;

use crate::ExecutorPluginConnectorProvider;

/// Resolves connector declarations from thread-selected executor plugins.
#[derive(Clone, Debug)]
pub struct SelectedExecutorConnectorProvider {
    connector_provider: ExecutorPluginConnectorProvider,
}

impl SelectedExecutorConnectorProvider {
    /// Creates a provider for already-bound selected capability roots.
    pub fn new() -> Self {
        Self {
            connector_provider: ExecutorPluginConnectorProvider,
        }
    }

    /// Resolves one immutable connector snapshot in selected-root order.
    pub async fn snapshot_for_bindings(
        &self,
        bindings: &SelectedCapabilityBindings,
    ) -> ConnectorSnapshot {
        let mut sources = Vec::new();
        let snapshot = bindings.resolve_all().await;

        for selected_root in snapshot.ready() {
            let Some(plugin) = selected_root.plugin() else {
                continue;
            };
            match self.connector_provider.load(selected_root).await {
                Ok(declarations) => sources.push(PluginConnectorSource::new(
                    plugin.selected_root_id(),
                    plugin.manifest().display_name(),
                    declarations,
                )),
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.selected_root().id,
                        error = %err,
                        "failed to load selected executor plugin connectors"
                    );
                }
            }
        }

        ConnectorSnapshot::from_plugin_sources(sources)
    }
}

impl ThreadExtensionInitContributor for SelectedExecutorConnectorProvider {
    fn initialize<'a>(&'a self, thread_init: &'a mut ExtensionDataInit) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if thread_init.get::<ConnectorSnapshot>().is_some() {
                return;
            }
            let Some(bindings) = thread_init.get::<SelectedCapabilityBindings>() else {
                return;
            };
            let snapshot = self.snapshot_for_bindings(bindings.as_ref()).await;
            thread_init.insert(snapshot);
        })
    }
}
