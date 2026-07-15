use codex_connectors_extension::ExecutorPluginConnectorProvider;
use codex_core::config::Config;
use codex_core_plugins::ExecutorPluginProvider;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use self::provider::ExecutorPluginMcpProvider;

mod provider;

/// Frozen MCP and connector declarations for one selected package.
///
/// Each server config retains the stable logical environment ID. Reconnection may replace the
/// concrete environment instance without changing that authority.
#[derive(Clone)]
struct SelectedPluginMetadata {
    plugin_id: String,
    plugin_display_name: String,
    servers: Vec<(String, codex_config::McpServerConfig)>,
    connector_ids: Vec<String>,
}

#[derive(Default)]
pub(crate) struct SelectedExecutorPluginMcpState {
    cache: Mutex<Vec<CachedSelectedRoot>>,
}

struct CachedSelectedRoot {
    root: SelectedCapabilityRoot,
    metadata: Option<SelectedPluginMetadata>,
}

pub(crate) struct SelectedExecutorPluginMcpContributor {
    plugin_provider: ExecutorPluginProvider,
    mcp_provider: ExecutorPluginMcpProvider,
    connector_provider: ExecutorPluginConnectorProvider,
}

impl SelectedExecutorPluginMcpContributor {
    pub(crate) fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            plugin_provider: ExecutorPluginProvider::new(Arc::clone(&environment_manager)),
            mcp_provider: ExecutorPluginMcpProvider,
            connector_provider: ExecutorPluginConnectorProvider,
        }
    }

    /// Returns metadata for one stable selected root.
    ///
    /// Successful resolution, including a root that is not a plugin or declares no capabilities,
    /// is cached until the thread state is dropped. Environment availability never invalidates
    /// this cache; it only controls whether the cached metadata is projected into a model step.
    #[tracing::instrument(name = "mcp.executor_plugin.metadata.load", skip_all)]
    async fn metadata_for_root(
        &self,
        state: &SelectedExecutorPluginMcpState,
        selected_root: &SelectedCapabilityRoot,
    ) -> Option<SelectedPluginMetadata> {
        if let Some(cached) = state
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|cached| cached.root == *selected_root)
        {
            return cached.metadata.clone();
        }

        let plugin = match self.plugin_provider.resolve_bound(selected_root).await {
            Ok(plugin) => plugin,
            Err(err) => {
                tracing::warn!(
                    selected_root = selected_root.id,
                    error = %err,
                    "failed to resolve selected executor plugin"
                );
                return None;
            }
        };
        let metadata = match plugin {
            Some(plugin) => {
                let servers = self.mcp_provider.load(&plugin).await.unwrap_or_else(|err| {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to load selected executor plugin MCP servers"
                    );
                    Vec::new()
                });
                let connector_ids = self
                    .connector_provider
                    .load(&plugin)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            selected_root = selected_root.id,
                            error = %err,
                            "failed to load selected executor plugin connectors"
                        );
                        Vec::new()
                    })
                    .into_iter()
                    .map(|declaration| declaration.connector_id.0)
                    .collect();
                Some(SelectedPluginMetadata {
                    plugin_id: plugin.plugin().selected_root_id().to_string(),
                    plugin_display_name: plugin.plugin().manifest().display_name().to_string(),
                    servers,
                    connector_ids,
                })
            }
            None => None,
        };
        let mut cache = state
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.iter().find(|cached| cached.root == *selected_root) {
            return cached.metadata.clone();
        }
        cache.push(CachedSelectedRoot {
            root: selected_root.clone(),
            metadata: metadata.clone(),
        });
        metadata
    }
}

impl McpServerContributor<Config> for SelectedExecutorPluginMcpContributor {
    fn id(&self) -> &'static str {
        "selected_executor_plugin_mcp"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let Some(thread_init) = context.thread_init() else {
                return Vec::new();
            };
            let Some(thread_store) = context.thread_store() else {
                return Vec::new();
            };
            let Some(selected_roots) = thread_init.get::<Vec<SelectedCapabilityRoot>>() else {
                return Vec::new();
            };
            let state = thread_store.get_or_init(SelectedExecutorPluginMcpState::default);
            let mut contributions = Vec::new();

            for (selection_order, selected_root) in selected_roots.iter().enumerate() {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                if context
                    .available_environment_ids()
                    .is_some_and(|available| {
                        !available
                            .iter()
                            .any(|available| available == environment_id)
                    })
                {
                    continue;
                }
                let Some(plugin) = self.metadata_for_root(&state, selected_root).await else {
                    continue;
                };
                let mut servers = plugin.servers.iter().cloned().collect::<HashMap<_, _>>();
                context
                    .config()
                    .apply_plugin_mcp_server_requirements(&plugin.plugin_id, &mut servers);
                let mut servers = servers.into_iter().collect::<Vec<_>>();
                servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                contributions.extend(servers.into_iter().map(|(name, config)| {
                    McpServerContribution::SelectedPlugin {
                        name,
                        plugin_id: plugin.plugin_id.clone(),
                        plugin_display_name: plugin.plugin_display_name.clone(),
                        selection_order,
                        config: Box::new(config),
                    }
                }));
                // Keep the package visible even when it contributes only skills.
                contributions.push(McpServerContribution::SelectedPluginPackage {
                    plugin_id: plugin.plugin_id,
                    plugin_display_name: plugin.plugin_display_name,
                    connector_ids: plugin.connector_ids,
                });
            }

            contributions
        })
    }
}
