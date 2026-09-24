use codex_config::types::PluginMcpServerConfig;
use codex_connectors_extension::PluginAppProvider;
use codex_core::config::Config;
use codex_core_plugins::loader::apply_configured_plugin_mcp_server_policies;
use codex_core_plugins::loader::configured_plugin_mcp_server_policies;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPlugin;
use codex_extension_api::SelectedPluginContribution;
use codex_features::Feature;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

use self::provider::PluginMcpProvider;
use crate::PluginsThreadState;
use crate::cloud_plugin::hosted_plugin_connectors;
use crate::plugin_contributor::PluginContributor;
use crate::plugin_contributor_state::CachedSelectedRoot;

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
    ) -> Option<SelectedPluginContribution> {
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
                Some(SelectedPluginContribution {
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
            let cloud_plugins_enabled = self.providers.cloud.is_some()
                && context.config().features.enabled(Feature::Plugins);
            let state = thread_store.get_or_init(PluginsThreadState::default);
            if !cloud_plugins_enabled {
                state.contributor_state().cloud_generation = None;
                return Vec::new();
            }
            // Core reads hosted connectors after finishing the executor registrations.
            state
                .cloud_catalog()
                .as_ref()
                .map(hosted_plugin_connectors)
                .unwrap_or_default()
        })
    }

    fn selected_plugins<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<SelectedPlugin<'a>>> {
        Box::pin(async move {
            let Some(thread_store) = context.thread_store() else {
                return Vec::new();
            };
            let state = thread_store.get_or_init(PluginsThreadState::default);
            if context.auth_changed() {
                // Clear the old account before executor loading can yield to other work.
                state.contributor_state().cloud_generation = None;
            }
            let selected_roots = context
                .ready_selected_capability_roots()
                .unwrap_or_default();
            // All plugins in this refresh share the same policy, but step-only readers never need it.
            let plugin_policies = Arc::new(OnceLock::new());
            let mut plugins = Vec::new();

            if let Some(snapshot) = context.executor_capability_discovery() {
                for root in snapshot.roots() {
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
                    let Some((manifest, plugin_files)) =
                        discovery::manifest_from_discovery(&root.selected_root, discovery)
                    else {
                        continue;
                    };
                    let plugin_policies = Arc::clone(&plugin_policies);
                    plugins.push(SelectedPlugin {
                        selected_root_id: root.selected_root.id.clone(),
                        plugin_id: root.selected_root.id.clone(),
                        mcp: Box::pin(async move {
                            let metadata = discovery::metadata_from_discovery(
                                &root.selected_root,
                                discovery,
                                plugin_files,
                                manifest,
                            );
                            project_metadata(
                                context.config(),
                                &root.selected_root.id,
                                &plugin_policies,
                                metadata,
                            )
                        }),
                    });
                }
            } else {
                for selected_root in selected_roots {
                    let Some(metadata) = self.metadata_for_root(&state, selected_root).await else {
                        continue;
                    };
                    let plugin_policies = Arc::clone(&plugin_policies);
                    plugins.push(SelectedPlugin {
                        selected_root_id: selected_root.id.clone(),
                        plugin_id: selected_root.id.clone(),
                        mcp: Box::pin(async move {
                            project_metadata(
                                context.config(),
                                &selected_root.id,
                                &plugin_policies,
                                metadata,
                            )
                        }),
                    });
                }
            }
            plugins
        })
    }
}

fn project_metadata(
    config: &Config,
    plugin_id: &str,
    plugin_policies: &OnceLock<HashMap<String, HashMap<String, PluginMcpServerConfig>>>,
    plugin: SelectedPluginContribution,
) -> SelectedPluginContribution {
    let mut servers = if config.features.enabled(Feature::Plugins) {
        plugin.servers.into_iter().collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    if !servers.is_empty() {
        if let Some(plugin_policy) = plugin_policies
            .get_or_init(|| configured_plugin_mcp_server_policies(&config.config_layer_stack))
            .get(plugin_id)
        {
            apply_configured_plugin_mcp_server_policies(plugin_policy, &mut servers);
        }
        config.apply_plugin_mcp_server_requirements(plugin_id, &mut servers);
    }
    let mut servers = servers.into_iter().collect::<Vec<_>>();
    servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    SelectedPluginContribution { servers, ..plugin }
}
