use codex_core::config::Config;
use codex_core_plugins::SelectedCapabilityActivation;
use codex_core_plugins::SelectedCapabilityBindings;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::ThreadExtensionInitContributor;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
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
    plugins: Mutex<HashMap<usize, Arc<OnceCell<Option<SelectedPluginMcpServers>>>>>,
}

impl SelectedExecutorPluginMcpState {
    fn plugin(&self, selection_order: usize) -> Arc<OnceCell<Option<SelectedPluginMcpServers>>> {
        Arc::clone(
            self.plugins
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(selection_order)
                .or_default(),
        )
    }
}

pub(crate) fn seed_thread_state(thread_init: &mut ExtensionDataInit) {
    thread_init.insert(SelectedExecutorPluginMcpState::default());
}

pub(crate) struct SelectedExecutorPluginMcpInitializer;

impl ThreadExtensionInitContributor for SelectedExecutorPluginMcpInitializer {
    fn initialize<'a>(&'a self, thread_init: &'a mut ExtensionDataInit) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if (thread_init.get::<SelectedCapabilityActivation>().is_some()
                || thread_init.get::<SelectedCapabilityBindings>().is_some())
                && thread_init
                    .get::<SelectedExecutorPluginMcpState>()
                    .is_none()
            {
                seed_thread_state(thread_init);
            }
        })
    }
}

pub(crate) struct SelectedExecutorPluginMcpContributor {
    mcp_provider: ExecutorPluginMcpProvider,
}

impl SelectedExecutorPluginMcpContributor {
    pub(crate) fn new() -> Self {
        Self {
            mcp_provider: ExecutorPluginMcpProvider,
        }
    }

    async fn resolve_selected_root(
        &self,
        selected_root: &codex_core_plugins::ResolvedSelectedCapabilityRoot,
    ) -> Option<SelectedPluginMcpServers> {
        let plugin = selected_root.plugin()?;
        match self.mcp_provider.load(selected_root).await {
            Ok(servers) => Some(SelectedPluginMcpServers {
                plugin_id: plugin.selected_root_id().to_string(),
                plugin_display_name: plugin.manifest().display_name().to_string(),
                selection_order: selected_root.selection_order(),
                servers,
            }),
            Err(err) => {
                tracing::warn!(
                    selected_root = selected_root.selected_root().id,
                    error = %err,
                    "failed to load selected executor plugin MCP servers"
                );
                None
            }
        }
    }

    async fn snapshot_for_selected_capabilities(
        &self,
        selected_capabilities: &codex_core_plugins::SelectedCapabilitySnapshot,
        state: &SelectedExecutorPluginMcpState,
    ) -> Vec<SelectedPluginMcpServers> {
        let mut plugins = Vec::new();
        for selected_root in selected_capabilities.ready() {
            let selection_order = selected_root.selection_order();
            let plugin = state
                .plugin(selection_order)
                .get_or_init(|| self.resolve_selected_root(selected_root))
                .await
                .clone();
            if let Some(plugin) = plugin {
                plugins.push(plugin);
            }
        }
        plugins
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
            let Some(state) = thread_init.get::<SelectedExecutorPluginMcpState>() else {
                tracing::warn!("selected executor plugin MCP state was not initialized");
                return Vec::new();
            };
            let selected_capabilities = match thread_init.get::<SelectedCapabilityActivation>() {
                Some(activation) => activation.snapshot().selected_capabilities().clone(),
                None => {
                    let Some(bindings) = thread_init.get::<SelectedCapabilityBindings>() else {
                        return Vec::new();
                    };
                    bindings.resolve_all().await
                }
            };
            let snapshot = self
                .snapshot_for_selected_capabilities(&selected_capabilities, state.as_ref())
                .await;
            let mut contributions = Vec::new();

            for plugin in &snapshot {
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
