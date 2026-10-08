use codex_config::types::PluginsConfigToml;
use codex_connectors_extension::PluginAppProvider;
use codex_core::config::Config;
use codex_core_plugins::ExecutorPluginProviderError;
use codex_core_plugins::PluginRootOwnership;
use codex_core_plugins::loader::apply_configured_plugin_mcp_server_policies;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::FileSystemSandboxContext;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::SelectedPlugin;
use codex_extension_api::SelectedPluginContribution;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;

use self::provider::PluginMcpProvider;
use crate::PluginsThreadState;
use crate::cloud_plugin::hosted_plugin_connectors;
use crate::plugin_contributor::PluginContributor;
use crate::plugin_contributor_state::CachedPluginMetadata;
use crate::plugin_contributor_state::CachedSelectedRoot;

mod discovery;
mod provider;

impl PluginContributor {
    async fn ownership_for_root(
        &self,
        state: &PluginsThreadState,
        selected_root: &SelectedCapabilityRoot,
        discovery: Option<&CapabilityRootDiscovery>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> Result<PluginRootOwnership, ExecutorPluginProviderError> {
        if let Some(ownership) = state
            .contributor_state()
            .executor_cache
            .iter()
            .find(|cached| {
                cached.root == *selected_root && cached.ownership_sandbox.as_deref() == sandbox
            })
            .and_then(|cached| cached.ownership)
        {
            return Ok(ownership);
        }
        let ownership = self
            .providers
            .executor
            .root_ownership(selected_root, discovery, sandbox)
            .await
            .inspect_err(|error| {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                tracing::warn!(
                    selected_root = selected_root.id,
                    environment_id,
                    error = %error,
                    "failed to resolve selected root plugin ownership"
                );
            })?;
        let mut state = state.contributor_state();
        if let Some(cached) = state
            .executor_cache
            .iter_mut()
            .find(|cached| cached.root == *selected_root)
        {
            if cached.ownership_sandbox.as_deref() == sandbox
                && let Some(ownership) = cached.ownership
            {
                return Ok(ownership);
            }
            cached.ownership = Some(ownership);
            cached.ownership_sandbox = sandbox.cloned().map(Box::new);
        } else {
            state.executor_cache.push(CachedSelectedRoot {
                root: selected_root.clone(),
                ownership: Some(ownership),
                ownership_sandbox: sandbox.cloned().map(Box::new),
                metadata: CachedPluginMetadata::Unloaded,
            });
        }
        Ok(ownership)
    }

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
        if let Some(CachedSelectedRoot {
            metadata: CachedPluginMetadata::Loaded(metadata),
            ..
        }) = state
            .contributor_state()
            .executor_cache
            .iter()
            .find(|cached| cached.root == *selected_root)
        {
            return metadata.clone();
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
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                Some(SelectedPluginContribution {
                    plugin_display_name: plugin.plugin().manifest().display_name().to_string(),
                    source_environment_id: environment_id.clone(),
                    servers,
                    connector_ids,
                })
            }
            None => None,
        };
        let mut state = state.contributor_state();
        let cache = &mut state.executor_cache;
        if let Some(cached) = cache
            .iter_mut()
            .find(|cached| cached.root == *selected_root)
        {
            if let CachedPluginMetadata::Loaded(metadata) = &cached.metadata {
                return metadata.clone();
            }
            cached.metadata = CachedPluginMetadata::Loaded(metadata.clone());
        } else {
            cache.push(CachedSelectedRoot {
                root: selected_root.clone(),
                ownership: None,
                ownership_sandbox: None,
                metadata: CachedPluginMetadata::Loaded(metadata.clone()),
            });
        }
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
        plugins_config: &'a PluginsConfigToml,
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
                    let CapabilityRootLocation::Environment { environment_id, .. } =
                        &root.selected_root.location;
                    // Ownership survives capability parse failures; an unresolved root cannot
                    // bypass a denying policy.
                    if !plugins_config.allows_plugin(&root.selected_root.id)
                        && discovery.error.is_none()
                        && !matches!(
                            self.ownership_for_root(
                                &state,
                                &root.selected_root,
                                Some(discovery),
                                snapshot.sandbox_contexts().get(environment_id),
                            )
                            .await,
                            Ok(PluginRootOwnership::Standalone)
                        )
                    {
                        plugins.push(denied_plugin(&root.selected_root));
                        continue;
                    }
                    let Some((manifest, plugin_files)) =
                        discovery::manifest_from_discovery(&root.selected_root, discovery)
                    else {
                        continue;
                    };
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
                                plugins_config,
                                &root.selected_root.id,
                                metadata,
                            )
                        }),
                    });
                }
            } else {
                for selected_root in selected_roots {
                    if !plugins_config.allows_plugin(&selected_root.id) {
                        if !matches!(
                            self.ownership_for_root(
                                &state,
                                selected_root,
                                /*discovery*/ None,
                                /*sandbox*/ None
                            )
                            .await,
                            Ok(PluginRootOwnership::Standalone)
                        ) {
                            plugins.push(denied_plugin(selected_root));
                        }
                        continue;
                    }
                    let Some(metadata) = self.metadata_for_root(&state, selected_root).await else {
                        continue;
                    };
                    plugins.push(SelectedPlugin {
                        selected_root_id: selected_root.id.clone(),
                        plugin_id: selected_root.id.clone(),
                        mcp: Box::pin(async move {
                            project_metadata(
                                context.config(),
                                plugins_config,
                                &selected_root.id,
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

fn denied_plugin(selected_root: &SelectedCapabilityRoot) -> SelectedPlugin<'static> {
    let CapabilityRootLocation::Environment { environment_id, .. } = &selected_root.location;
    let metadata = SelectedPluginContribution {
        plugin_display_name: selected_root.id.clone(),
        source_environment_id: environment_id.clone(),
        servers: Vec::new(),
        connector_ids: Vec::new(),
    };
    SelectedPlugin {
        selected_root_id: selected_root.id.clone(),
        plugin_id: selected_root.id.clone(),
        mcp: Box::pin(async move { metadata }),
    }
}

fn project_metadata(
    config: &Config,
    plugins_config: &PluginsConfigToml,
    plugin_id: &str,
    plugin: SelectedPluginContribution,
) -> SelectedPluginContribution {
    let mut servers = if config.features.enabled(Feature::Plugins) {
        plugin.servers.into_iter().collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    if !servers.is_empty() {
        if let Some(plugin_policy) = plugins_config.plugins.get(plugin_id) {
            apply_configured_plugin_mcp_server_policies(&plugin_policy.mcp_servers, &mut servers);
        }
        config.apply_plugin_mcp_server_requirements(plugin_id, &mut servers);
    }
    let mut servers = servers.into_iter().collect::<Vec<_>>();
    servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    SelectedPluginContribution { servers, ..plugin }
}
