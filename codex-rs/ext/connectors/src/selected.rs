use codex_connectors::ConnectorSnapshot;
use codex_connectors::ConnectorSnapshotState;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::SelectedCapabilityActivation;
use codex_core_plugins::SelectedCapabilityBindings;
use codex_core_plugins::SelectedCapabilitySnapshot;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::RuntimeSnapshotContributor;
use codex_extension_api::ThreadExtensionInitContributor;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use tokio::sync::OnceCell;

use crate::ExecutorPluginConnectorProvider;

#[derive(Default)]
struct SelectedExecutorConnectorCache {
    sources: Mutex<HashMap<usize, Arc<OnceCell<Option<PluginConnectorSource>>>>>,
}

impl SelectedExecutorConnectorCache {
    fn source(&self, selection_order: usize) -> Arc<OnceCell<Option<PluginConnectorSource>>> {
        Arc::clone(
            self.sources
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(selection_order)
                .or_default(),
        )
    }
}

/// Resolves connector declarations from thread-selected executor plugins.
#[derive(Clone, Debug, Default)]
pub struct SelectedExecutorConnectorProvider {
    connector_provider: ExecutorPluginConnectorProvider,
}

impl SelectedExecutorConnectorProvider {
    /// Creates a provider for already-bound selected capability roots.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves one immutable connector snapshot after all selected roots settle.
    ///
    /// Thread initialization uses generation snapshots instead so a pending executor does not
    /// block session startup. This method retains the explicit wait-for-all behavior for callers
    /// that request a complete snapshot directly.
    pub async fn snapshot_for_bindings(
        &self,
        bindings: &SelectedCapabilityBindings,
    ) -> ConnectorSnapshot {
        self.snapshot_for_selected_capabilities(
            &bindings.resolve_all().await,
            &SelectedExecutorConnectorCache::default(),
        )
        .await
    }

    /// Resolves one immutable connector snapshot in selected-root order.
    async fn snapshot_for_selected_capabilities(
        &self,
        selected_capabilities: &SelectedCapabilitySnapshot,
        cache: &SelectedExecutorConnectorCache,
    ) -> ConnectorSnapshot {
        for selected_root in selected_capabilities.ready() {
            let selection_order = selected_root.selection_order();
            cache
                .source(selection_order)
                .get_or_init(|| async {
                    let plugin = selected_root.plugin()?;
                    match self.connector_provider.load(selected_root).await {
                        Ok(declarations) => Some(PluginConnectorSource::new(
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
                            None
                        }
                    }
                })
                .await;
        }

        ConnectorSnapshot::from_plugin_sources(selected_capabilities.ready().filter_map(
            |selected_root| {
                cache
                    .source(selected_root.selection_order())
                    .get()
                    .cloned()
                    .flatten()
            },
        ))
    }
}

impl ThreadExtensionInitContributor for SelectedExecutorConnectorProvider {
    fn initialize<'a>(&'a self, thread_init: &'a mut ExtensionDataInit) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(snapshot) = thread_init.get::<ConnectorSnapshot>() {
                let snapshot = snapshot.as_ref().clone();
                match thread_init.get::<ConnectorSnapshotState>() {
                    Some(state) => state.publish(snapshot),
                    None => {
                        thread_init.insert(ConnectorSnapshotState::new(snapshot));
                    }
                }
                return;
            }

            let Some(activation) = thread_init.get::<SelectedCapabilityActivation>() else {
                let Some(bindings) = thread_init.get::<SelectedCapabilityBindings>() else {
                    return;
                };
                let snapshot = self.snapshot_for_bindings(bindings.as_ref()).await;
                thread_init.insert(snapshot.clone());
                thread_init.insert(ConnectorSnapshotState::new(snapshot));
                return;
            };
            if thread_init.get::<ConnectorSnapshotState>().is_none() {
                thread_init.insert(ConnectorSnapshotState::default());
            }
            let Some(connector_state) = thread_init.get::<ConnectorSnapshotState>() else {
                return;
            };
            if thread_init
                .get::<SelectedExecutorConnectorCache>()
                .is_none()
            {
                thread_init.insert(SelectedExecutorConnectorCache::default());
            }
            let Some(cache) = thread_init.get::<SelectedExecutorConnectorCache>() else {
                return;
            };
            let selected_capabilities = activation.snapshot().selected_capabilities().clone();
            let connector_snapshot = self
                .snapshot_for_selected_capabilities(&selected_capabilities, cache.as_ref())
                .await;
            connector_state.publish(connector_snapshot);
        })
    }
}

impl RuntimeSnapshotContributor for SelectedExecutorConnectorProvider {
    fn prepare<'a>(&'a self, candidate: &'a mut ExtensionDataInit) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Some(activation) = candidate.get::<SelectedCapabilityActivation>() else {
                return;
            };
            candidate.insert(ConnectorSnapshotState::default());
            let Some(connector_state) = candidate.get::<ConnectorSnapshotState>() else {
                return;
            };
            if let Some(snapshot) = candidate.get::<ConnectorSnapshot>() {
                connector_state.publish(snapshot.as_ref().clone());
                return;
            }
            let Some(cache) = candidate.get::<SelectedExecutorConnectorCache>() else {
                return;
            };
            let selected_capabilities = activation.snapshot().selected_capabilities().clone();
            let connector_snapshot = self
                .snapshot_for_selected_capabilities(&selected_capabilities, cache.as_ref())
                .await;
            connector_state.publish(connector_snapshot);
        })
    }

    fn commit(&self, candidate: &ExtensionDataInit, active: &ExtensionDataInit) {
        let Some(candidate_state) = candidate.get::<ConnectorSnapshotState>() else {
            return;
        };
        let Some(active_state) = active.get::<ConnectorSnapshotState>() else {
            return;
        };
        active_state.publish(candidate_state.snapshot());
    }
}
