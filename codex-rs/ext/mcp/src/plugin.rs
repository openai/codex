use codex_config::types::PluginMcpServerConfig;
use codex_connectors_extension::PluginAppProvider;
use codex_core::config::Config;
use codex_core_plugins::loader::apply_configured_plugin_mcp_server_policies;
use codex_core_plugins::loader::configured_plugin_mcp_server_policies;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;

use self::provider::PluginMcpProvider;
use crate::PluginsThreadState;
use crate::cloud_plugin::CloudPluginMetadata;
use crate::cloud_plugin::catalog_to_metadata;
use crate::plugin_contributor::PluginContributor;
use crate::plugin_contributor_state::CachedSelectedRoot;
use crate::plugin_contributor_state::SelectedPluginMetadata;

mod discovery;
mod provider;

impl PluginContributor {
    /// Returns metadata for one stable selected root.
    ///
    /// Successful resolution, including a root that is not a plugin or declares no capabilities,
    /// is cached until the thread state is dropped. Environment availability never invalidates
    /// this cache; it only controls whether the cached metadata is projected into a model step.
    #[tracing::instrument(name = "mcp.plugin.metadata.load", skip_all)]
    async fn metadata_for_root(
        &self,
        state: &PluginsThreadState,
        selected_root: &SelectedCapabilityRoot,
    ) -> Option<SelectedPluginMetadata> {
        if let Some(cached) = state
            .contributor_state()
            .executor_cache
            .iter()
            .find(|cached| cached.root == *selected_root)
        {
            return cached.metadata.clone();
        }

        let plugin = match self.providers.executor.resolve_bound(selected_root).await {
            Ok(plugin) => plugin,
            Err(err) => {
                tracing::warn!(
                    selected_root = selected_root.id,
                    error = %err,
                    "failed to resolve selected plugin"
                );
                return None;
            }
        };
        let metadata = match plugin {
            Some(plugin) => {
                // MCP server and app declarations are separate
                // executor-owned files. Read them together so a remote environment only
                // pays for the slower read instead of both reads back-to-back.
                let (servers, app_declarations) = tokio::join!(
                    PluginMcpProvider.load(&plugin),
                    PluginAppProvider.load(&plugin)
                );
                let servers = servers.unwrap_or_else(|err| {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to load selected plugin MCP servers"
                    );
                    Vec::new()
                });
                let connector_ids = app_declarations
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            selected_root = selected_root.id,
                            error = %err,
                            "failed to load selected plugin apps"
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
        let mut state = state.contributor_state();
        let cache = &mut state.executor_cache;
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

impl McpServerContributor<Config> for PluginContributor {
    fn id(&self) -> &'static str {
        "plugin"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let Some(thread_store) = context.thread_store() else {
                return Vec::new();
            };
            // Cloud projection is gated independently; executor projection remains unchanged.
            let cloud_plugins_enabled = self.providers.cloud.is_some()
                && context.config().features.enabled(Feature::Plugins);
            let state = thread_store.get_or_init(PluginsThreadState::default);
            if !cloud_plugins_enabled || context.auth_changed() {
                // Clear stale cloud metadata before projecting a replacement runtime.
                state.contributor_state().cloud_generation = None;
            }
            let selected_roots = context
                .ready_selected_capability_roots()
                .unwrap_or_default();
            let plugin_policies =
                configured_plugin_mcp_server_policies(&context.config().config_layer_stack);
            let mut contributions = Vec::new();

            if let Some(snapshot) = context.executor_capability_discovery() {
                for (selection_order, root) in snapshot.roots().iter().enumerate() {
                    let discovery = match &root.result {
                        Ok(discovery) => discovery.as_ref(),
                        Err(error) => {
                            tracing::warn!(
                                selected_root = root.selected_root.id,
                                error,
                                "exec-server capability discovery request failed"
                            );
                            continue;
                        }
                    };
                    let Some(plugin) =
                        discovery::metadata_from_discovery(&root.selected_root, discovery)
                    else {
                        continue;
                    };
                    contributions.extend(project_metadata(
                        context.config(),
                        plugin_policies.get(&plugin.plugin_id),
                        selection_order,
                        &root.selected_root.id,
                        plugin,
                    ));
                }
            } else {
                for (selection_order, selected_root) in selected_roots.iter().enumerate() {
                    let Some(plugin) = self.metadata_for_root(&state, selected_root).await else {
                        continue;
                    };
                    contributions.extend(project_metadata(
                        context.config(),
                        plugin_policies.get(&plugin.plugin_id),
                        selection_order,
                        &selected_root.id,
                        plugin,
                    ));
                }
            }

            // V1 has no remote plugin identities, so cloud packages are additive for now.
            // Clone the current snapshot after executor loading and release the lock before projection.
            if cloud_plugins_enabled && let Some(catalog) = state.cloud_catalog() {
                for CloudPluginMetadata {
                    selection_order,
                    selected_root_id,
                    metadata,
                } in catalog_to_metadata(&catalog)
                {
                    contributions.extend(project_metadata(
                        context.config(),
                        /*plugin_policy*/ None,
                        selection_order,
                        &selected_root_id,
                        metadata,
                    ));
                }
            }
            contributions
        })
    }
}

fn project_metadata(
    config: &Config,
    plugin_policy: Option<&HashMap<String, PluginMcpServerConfig>>,
    selection_order: usize,
    selected_root_id: &str,
    plugin: SelectedPluginMetadata,
) -> Vec<McpServerContribution> {
    let mut servers = if config.features.enabled(Feature::Plugins) {
        plugin.servers.iter().cloned().collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    if let Some(plugin_policy) = plugin_policy {
        apply_configured_plugin_mcp_server_policies(plugin_policy, &mut servers);
    }
    config.apply_plugin_mcp_server_requirements(&plugin.plugin_id, &mut servers);
    let mut servers = servers.into_iter().collect::<Vec<_>>();
    servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut contributions = servers
        .into_iter()
        .map(|(name, config)| McpServerContribution::SelectedPlugin {
            name,
            plugin_id: plugin.plugin_id.clone(),
            plugin_display_name: plugin.plugin_display_name.clone(),
            selection_order,
            config: Box::new(config),
        })
        .collect::<Vec<_>>();
    // Keep the package visible even when it contributes only skills.
    contributions.push(McpServerContribution::SelectedPluginPackage {
        selected_root_id: selected_root_id.to_owned(),
        plugin_id: plugin.plugin_id,
        plugin_display_name: plugin.plugin_display_name,
        connector_ids: plugin.connector_ids,
    });
    contributions
}
