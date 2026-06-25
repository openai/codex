use codex_core::config::Config;
use codex_core_plugins::ExecutorPluginRuntime;
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

/// The runtime declarations frozen for one selected package at thread start.
#[derive(Clone)]
struct SelectedPluginRuntime {
    selection_order: usize,
    runtime: ExecutorPluginRuntime,
}

#[derive(Default)]
pub(crate) struct SelectedExecutorPluginMcpState {
    snapshot: OnceCell<Vec<SelectedPluginRuntime>>,
}

pub(crate) fn seed_thread_state(thread_init: &mut ExtensionDataInit) {
    thread_init.insert(SelectedExecutorPluginMcpState::default());
}

pub(crate) struct SelectedExecutorPluginMcpContributor {
    environment_manager: Arc<EnvironmentManager>,
}

impl SelectedExecutorPluginMcpContributor {
    pub(crate) fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
        }
    }

    async fn resolve_snapshot(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> Vec<SelectedPluginRuntime> {
        let mut snapshot = Vec::new();
        let resolved_roots = self
            .environment_manager
            .bind_selected_capability_roots(selected_roots);

        for (selection_order, resolved_root) in resolved_roots.iter().enumerate() {
            let selected_root = resolved_root.selected_root();
            match ExecutorPluginRuntime::project(resolved_root).await {
                Ok(Some(runtime)) => snapshot.push(SelectedPluginRuntime {
                    selection_order,
                    runtime,
                }),
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to project selected executor plugin runtime"
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

            for selected in snapshot {
                let plugin = selected.runtime.plugin();
                let plugin_id = plugin.selected_root_id();
                let mut servers = selected
                    .runtime
                    .mcp_servers()
                    .iter()
                    .cloned()
                    .collect::<HashMap<_, _>>();
                context
                    .config()
                    .apply_plugin_mcp_server_requirements(plugin_id, &mut servers);
                let mut servers = servers.into_iter().collect::<Vec<_>>();
                servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                contributions.extend(servers.into_iter().map(|(name, config)| {
                    McpServerContribution::SelectedPlugin {
                        name,
                        plugin_id: plugin_id.to_string(),
                        plugin_display_name: plugin.manifest().display_name().to_string(),
                        selection_order: selected.selection_order,
                        config: Box::new(config),
                    }
                }));
            }

            contributions
        })
    }
}
