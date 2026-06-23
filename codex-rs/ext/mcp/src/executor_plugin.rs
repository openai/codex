use codex_core::config::Config;
use codex_core_plugins::ExecutorPluginProvider;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::OnceCell;

use self::provider::ExecutorPluginMcpProvider;

mod provider;

/// Frozen MCP declarations for one selected package.
///
/// Each server config retains the stable logical environment ID. Reconnection may replace the
/// concrete environment instance without changing that authority.
#[derive(Clone)]
struct SelectedPluginMcpServers {
    plugin_id: String,
    plugin_display_name: String,
    selection_order: usize,
    servers: Vec<(String, codex_config::McpServerConfig)>,
}

#[derive(Default)]
pub(crate) struct SelectedExecutorPluginMcpState {
    snapshot: OnceCell<Vec<SelectedPluginMcpServers>>,
}

pub(crate) fn seed_thread_state(thread_init: &mut ExtensionDataInit) {
    thread_init.insert(SelectedExecutorPluginMcpState::default());
}

pub(crate) struct SelectedExecutorPluginMcpContributor {
    plugin_provider: ExecutorPluginProvider,
    mcp_provider: ExecutorPluginMcpProvider,
}

impl SelectedExecutorPluginMcpContributor {
    pub(crate) fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            plugin_provider: ExecutorPluginProvider::new(Arc::clone(&environment_manager)),
            mcp_provider: ExecutorPluginMcpProvider,
        }
    }

    async fn resolve_snapshot(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> Vec<SelectedPluginMcpServers> {
        let mut snapshot = Vec::new();

        for (selection_order, selected_root) in selected_roots.iter().enumerate() {
            let plugin = match self.plugin_provider.resolve_bound(selected_root).await {
                Ok(Some(plugin)) => plugin,
                Ok(None) => continue,
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to resolve selected executor plugin for MCP discovery"
                    );
                    continue;
                }
            };
            match self.mcp_provider.load(&plugin).await {
                Ok(servers) => snapshot.push(SelectedPluginMcpServers {
                    plugin_id: plugin.plugin().selected_root_id().to_string(),
                    plugin_display_name: plugin.plugin().manifest().display_name().to_string(),
                    selection_order,
                    servers,
                }),
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to load selected executor plugin MCP servers"
                    );
                }
            }
        }

        snapshot
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
            let Some(selected_roots) = thread_init.get::<Vec<SelectedCapabilityRoot>>() else {
                return Vec::new();
            };
            let Some(state) = thread_init.get::<SelectedExecutorPluginMcpState>() else {
                tracing::warn!("selected executor plugin MCP state was not initialized");
                return Vec::new();
            };
            let snapshot = state
                .snapshot
                .get_or_init(|| self.resolve_snapshot(selected_roots.as_ref()))
                .await;
            let mut contributions = Vec::new();

            for plugin in snapshot {
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
                        selection_order: plugin.selection_order,
                        config: Box::new(config),
                    }
                }));
            }

            contributions
        })
    }
}
