use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use crate::config::Config;
use crate::mcp_tool_exposure::SharedMcpDefinitions;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::types::PluginsConfigToml;
use codex_connectors::ConnectorRuntimeManager;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::PluginIdentity as CatalogPluginIdentity;
use codex_core_plugins::PluginsManager;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::SelectedPlugin;
use codex_extension_api::SelectedPluginIdentity;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::SessionIsolation;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::McpEnvironmentAuthority;
use codex_mcp::McpPluginAttribution;
use codex_mcp::McpServerRegistration;
use codex_mcp::McpToolCatalogCache;
use codex_mcp::ToolInfo;
use codex_mcp::codex_apps_mcp_server_config;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;
use codex_plugin::AppConnectorId;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnEnvironmentSelection;

const LEGACY_CODEX_APPS_REGISTRATION_ID: &str = "legacy_codex_apps";

/// Thread-local policy that excludes all MCP servers, including runtime contributions.
/// Insert into `StartThreadOptions::thread_extension_init` before starting the thread.
pub struct McpServersDisabled;

/// MCP configuration and capability availability derived from the same inputs.
#[derive(Clone)]
pub(crate) struct McpRuntimeProjection {
    pub(crate) config: McpConfig,
    pub(crate) plugins_available: bool,
    pub(crate) selected_plugins: SelectedPluginSnapshot,
}

pub(crate) enum McpEnvironmentScope<'a> {
    /// Controller-level operations without an associated thread.
    HostOnly,
    /// Every thread selection captured for this operation, including unavailable configurations.
    Selected(&'a [TurnEnvironmentSelection]),
}

impl<'a> McpEnvironmentScope<'a> {
    pub(crate) fn authority_for(&self, environment_id: &str) -> McpEnvironmentAuthority<'a> {
        let Self::Selected(selections) = *self else {
            return McpEnvironmentAuthority::Unrestricted;
        };
        let Some(selection) = selections
            .iter()
            .find(|selection| selection.environment_id == environment_id)
        else {
            return if environment_id == DEFAULT_MCP_SERVER_ENVIRONMENT_ID {
                McpEnvironmentAuthority::Unrestricted
            } else {
                McpEnvironmentAuthority::SelectedPluginsOnly
            };
        };
        match &selection.config {
            EnvironmentConfigState::FromThread => McpEnvironmentAuthority::Unrestricted,
            EnvironmentConfigState::Pending | EnvironmentConfigState::Failed(_) => {
                McpEnvironmentAuthority::Unavailable
            }
            EnvironmentConfigState::Ready(config) => config
                .mcp_policy
                .as_ref()
                .map_or(McpEnvironmentAuthority::Unrestricted, |policy| {
                    McpEnvironmentAuthority::Restricted(policy)
                }),
        }
    }
}

pub(crate) struct McpThreadIdentity<'a> {
    pub(crate) auth_changed: bool,
    pub(crate) session_source: &'a SessionSource,
    pub(crate) originator: &'a str,
    pub(crate) environments: McpEnvironmentScope<'a>,
    pub(crate) disabled_plugin_ids: &'a [String],
}

enum OrderedMcpOverlay {
    Set(Box<McpServerRegistration>),
    Remove {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
    },
}

#[derive(Clone)]
pub struct McpManager {
    connection_pool: codex_mcp::McpConnectionPool,
    plugins_manager: Arc<PluginsManager>,
    extensions: Arc<ExtensionRegistry<Config>>,
    codex_apps_tools_cache: ConnectorRuntimeManager<ToolInfo>,
    tool_catalog_cache: McpToolCatalogCache,
    shared_definitions: Arc<SharedMcpDefinitions>,
    ema_state: Arc<codex_mcp::EmaState>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self::new_with_extensions(
            plugins_manager,
            codex_extension_api::empty_extension_registry(),
            ConnectorRuntimeManager::default(),
        )
    }

    /// Creates a manager that resolves host-installed MCP contributions.
    pub fn new_with_extensions(
        plugins_manager: Arc<PluginsManager>,
        extensions: Arc<ExtensionRegistry<Config>>,
        codex_apps_tools_cache: ConnectorRuntimeManager<ToolInfo>,
    ) -> Self {
        Self {
            connection_pool: codex_mcp::McpConnectionPool::default(),
            plugins_manager,
            extensions,
            codex_apps_tools_cache,
            tool_catalog_cache: McpToolCatalogCache::default(),
            shared_definitions: Arc::new(SharedMcpDefinitions::default()),
            ema_state: Arc::default(),
        }
    }

    pub fn codex_apps_tools_cache(&self) -> ConnectorRuntimeManager<ToolInfo> {
        self.codex_apps_tools_cache.clone()
    }

    pub fn tool_catalog_cache(&self) -> McpToolCatalogCache {
        self.tool_catalog_cache.clone()
    }

    pub(crate) fn connection_pool(&self) -> codex_mcp::McpConnectionPool {
        self.connection_pool.clone()
    }

    pub(crate) fn shared_definitions(&self) -> &SharedMcpDefinitions {
        &self.shared_definitions
    }

    /// Returns the MCP config after applying compatibility built-ins and
    /// runtime-only extension overlays.
    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        self.runtime_config_with_context(
            McpServerContributionContext::global(config),
            // Threadless discovery and control-plane paths have no effective thread
            // originator; active-thread tool calls use runtime_config_for_step below.
            /*originator*/
            None,
            McpEnvironmentScope::HostOnly,
            &[],
        )
        .await
        .config
    }

    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(name = "mcp.runtime_config.project_for_step", skip_all)]
    pub(crate) async fn runtime_config_for_step(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        identity: McpThreadIdentity<'_>,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
        executor_capability_discovery: Option<&ExecutorCapabilityDiscoverySnapshot>,
        executor_capability_discovery_v2: Option<&[ExecutorCapabilityDiscoveryV2Snapshot]>,
    ) -> McpRuntimeProjection {
        let mut context = McpServerContributionContext::for_step(
            config,
            thread_init,
            thread_store,
            identity.originator,
            ready_selected_capability_roots,
            executor_capability_discovery,
            executor_capability_discovery_v2,
        )
        .with_session_source(identity.session_source)
        .with_auth_changed(identity.auth_changed);
        if let McpEnvironmentScope::Selected(environments) = &identity.environments {
            context = context.with_selected_environments(environments);
        }
        self.runtime_config_with_context(
            context,
            Some(identity.originator),
            identity.environments,
            identity.disabled_plugin_ids,
        )
        .await
    }

    /// Resolves only plugins from this step's ready roots, independent of shared MCP refresh.
    pub(crate) async fn selected_plugins_for_step(
        &self,
        context: McpServerContributionContext<'_, Config>,
        plugins_config: &PluginsConfigToml,
        disabled_plugin_ids: &[String],
    ) -> SelectedPluginSnapshot {
        let mut selected = SelectedPluginSnapshot {
            disabled_plugin_ids: disabled_plugin_ids.to_vec(),
            ..Default::default()
        };
        let available_roots = available_plugin_roots(context);
        // Check hosted plugins even when no executor roots are selected.
        for contributor in self.extensions.mcp_server_contributors() {
            for SelectedPlugin {
                selected_root_id,
                plugin_id,
                identity,
                enabled,
                ..
            } in contributor.selected_plugins(context, plugins_config).await
            {
                if !available_roots.contains(&selected_root_id) {
                    continue;
                }
                if !enabled
                    || !context.config().features.enabled(Feature::Plugins)
                    || !plugins_config.allows_plugin(&plugin_id)
                    || disabled_plugin_ids.contains(&plugin_id)
                    || disabled_plugin_ids.iter().any(|id| id == identity.as_str())
                {
                    selected.disabled_plugin_roots.push(selected_root_id);
                } else {
                    selected.plugins.push(SelectedPluginIdentity {
                        selected_root_id: Some(selected_root_id),
                        plugin_id: identity,
                    });
                }
            }
            if context.config().features.enabled(Feature::Plugins) {
                for plugin_id in contributor.hosted_plugin_ids(context) {
                    if !disabled_plugin_ids.contains(&plugin_id) {
                        selected.plugins.push(SelectedPluginIdentity {
                            selected_root_id: None,
                            plugin_id: CatalogPluginIdentity::Remote {
                                remote_plugin_id: plugin_id,
                            },
                        });
                    }
                }
            }
        }
        selected
    }

    async fn runtime_config_with_context(
        &self,
        context: McpServerContributionContext<'_, Config>,
        originator: Option<&str>,
        environment_scope: McpEnvironmentScope<'_>,
        disabled_plugin_ids: &[String],
    ) -> McpRuntimeProjection {
        let config = context.config();
        if context
            .thread_init()
            .is_some_and(|init| init.get::<McpServersDisabled>().is_some())
        {
            let mut mcp_config = config.to_mcp_config_with_loaded_plugins(&Default::default(), []);
            mcp_config.mcp_server_catalog = Default::default();
            mcp_config.apps_enabled = false;
            return McpRuntimeProjection {
                config: mcp_config,
                plugins_available: false,
                selected_plugins: Default::default(),
            };
        }
        let plugins_config = &config.plugins;
        let mut selected_plugin_connector_sources = Vec::new();
        let mut selected_plugin_registrations = Vec::new();
        let mut selected_plugins = Vec::new();
        let mut disabled_plugin_roots = Vec::new();
        let mut suppressed_plugin_ids = disabled_plugin_ids.to_vec();
        let mut overlays = Vec::new();
        // A contributor can emit multiple ordered actions, so order each action globally rather
        // than enumerating contributors.
        let mut contribution_order = 0;
        let mut plugin_selection_order = 0;
        for contributor in self.extensions.mcp_server_contributors() {
            // Finish executor plugins before asking for hosted contributions, which may change
            // accounts while an executor is still loading.
            for SelectedPlugin {
                selected_root_id,
                plugin_id,
                identity,
                enabled,
                mcp,
            } in contributor.selected_plugins(context, plugins_config).await
            {
                if !enabled || !plugins_config.allows_plugin(&plugin_id) {
                    disabled_plugin_roots.push(selected_root_id);
                    suppressed_plugin_ids.push(plugin_id);
                    continue;
                }
                let selection_order = plugin_selection_order;
                plugin_selection_order += 1;
                let disabled = disabled_plugin_ids.contains(&plugin_id)
                    || disabled_plugin_ids.iter().any(|id| id == identity.as_str());
                if !config.features.enabled(Feature::Plugins) || disabled {
                    disabled_plugin_roots.push(selected_root_id);
                    suppressed_plugin_ids.push(plugin_id.clone());
                } else {
                    selected_plugins.push(SelectedPluginIdentity {
                        selected_root_id: Some(selected_root_id),
                        plugin_id: identity,
                    });
                }
                let contribution = mcp.await;
                if !disabled {
                    for (name, server) in contribution.servers {
                        selected_plugin_registrations.push(
                            McpServerRegistration::from_selected_plugin(
                                name,
                                McpPluginAttribution::new(
                                    plugin_id.clone(),
                                    contribution.plugin_display_name.clone(),
                                ),
                                selection_order,
                                &contribution.source_environment_id,
                                server,
                            ),
                        );
                    }
                }
                // Disabled plugins still tell Apps which connectors to hide.
                if config.features.enabled(Feature::Plugins)
                    && !contribution.connector_ids.is_empty()
                {
                    selected_plugin_connector_sources.push(
                        PluginConnectorSource::from_connector_ids(
                            plugin_id,
                            contribution.plugin_display_name,
                            contribution.connector_ids.into_iter().map(AppConnectorId),
                        ),
                    );
                }
            }

            for contribution in contributor.contribute(context).await {
                match contribution {
                    McpServerContribution::Set { name, config } => {
                        overlays.push(OrderedMcpOverlay::Set(Box::new(
                            McpServerRegistration::from_extension(
                                name,
                                contributor.id(),
                                contribution_order,
                                *config,
                            ),
                        )));
                    }
                    McpServerContribution::SetWithProtocolMode {
                        name,
                        config,
                        protocol_mode,
                    } => {
                        overlays.push(OrderedMcpOverlay::Set(Box::new(
                            McpServerRegistration::from_extension(
                                name,
                                contributor.id(),
                                contribution_order,
                                *config,
                            )
                            .with_protocol_mode(protocol_mode),
                        )));
                    }
                    McpServerContribution::HostedApps {
                        config,
                        protocol_mode,
                    } => {
                        let mut registration = McpServerRegistration::from_hosted_apps(
                            contributor.id(),
                            contribution_order,
                            *config,
                        );
                        if let Some(protocol_mode) = protocol_mode {
                            registration = registration.with_protocol_mode(protocol_mode);
                        }
                        overlays.push(OrderedMcpOverlay::Set(Box::new(registration)));
                    }
                    McpServerContribution::HostedPluginConnectors {
                        plugin_id,
                        plugin_display_name,
                        connector_ids,
                    } => {
                        if config.features.enabled(Feature::Plugins) {
                            if !disabled_plugin_ids.contains(&plugin_id) {
                                selected_plugins.push(SelectedPluginIdentity {
                                    selected_root_id: None,
                                    plugin_id: CatalogPluginIdentity::Remote {
                                        remote_plugin_id: plugin_id.clone(),
                                    },
                                });
                            }
                            if !connector_ids.is_empty() {
                                selected_plugin_connector_sources.push(
                                    PluginConnectorSource::from_connector_ids(
                                        plugin_id,
                                        plugin_display_name,
                                        connector_ids.into_iter().map(AppConnectorId),
                                    ),
                                );
                            }
                        }
                    }
                    McpServerContribution::Remove { name } => {
                        overlays.push(OrderedMcpOverlay::Remove {
                            contributor_id: contributor.id(),
                            contribution_order,
                            name,
                        });
                    }
                }
                contribution_order += 1;
            }
        }

        let loaded_plugins = self
            .plugins_manager
            .plugins_for_config(&config.plugins_config_input())
            .await;
        let host_plugin_connector_sources = loaded_plugins
            .capability_summaries()
            .iter()
            .map(PluginConnectorSource::from);
        let connector_snapshot = if config.features.enabled(Feature::Plugins) {
            self.plugins_manager.connector_snapshot(
                host_plugin_connector_sources.chain(selected_plugin_connector_sources),
                disabled_plugin_ids,
            )
        } else {
            Default::default()
        };
        let loaded_plugins = loaded_plugins.without_plugins(disabled_plugin_ids);
        let plugins_available =
            !selected_plugins.is_empty() || !loaded_plugins.capability_summaries().is_empty();
        let mut mcp_config = config
            .to_mcp_config_with_loaded_plugins(&loaded_plugins, selected_plugin_registrations);
        mcp_config.ema_state = Arc::clone(&self.ema_state);
        let mut catalog = mcp_config.mcp_server_catalog.to_builder();
        // Isolated sessions do not inherit the implicit Apps connection.
        if mcp_config.apps_enabled
            && context
                .thread_init()
                .and_then(codex_extension_api::ExtensionDataInit::get::<SessionIsolation>)
                .as_deref()
                != Some(&SessionIsolation::Isolated)
        {
            catalog.register(McpServerRegistration::from_compatibility(
                CODEX_APPS_MCP_SERVER_NAME.to_string(),
                LEGACY_CODEX_APPS_REGISTRATION_ID,
                codex_apps_mcp_server_config(
                    &mcp_config.chatgpt_base_url,
                    mcp_config.apps_mcp_product_sku.as_deref(),
                    originator,
                ),
            ));
        } else {
            catalog.remove_compatibility(
                CODEX_APPS_MCP_SERVER_NAME.to_string(),
                LEGACY_CODEX_APPS_REGISTRATION_ID,
            );
        }

        for overlay in overlays {
            match overlay {
                OrderedMcpOverlay::Set(registration) => catalog.register(*registration),
                OrderedMcpOverlay::Remove {
                    contributor_id,
                    contribution_order,
                    name,
                } => catalog.remove_extension(name, contributor_id, contribution_order),
            }
        }
        let catalog = catalog.build_with_environment_authority(|environment_id| {
            environment_scope.authority_for(environment_id)
        });
        for conflict in catalog.conflicts() {
            tracing::warn!(
                server = conflict.name,
                outcome = ?conflict.outcome,
                contenders = ?conflict.contenders,
                "conflicting MCP server actions; using resolved catalog outcome"
            );
        }
        mcp_config.mcp_server_catalog = catalog;
        mcp_config.connector_snapshot = connector_snapshot;
        McpRuntimeProjection {
            config: mcp_config,
            plugins_available,
            selected_plugins: SelectedPluginSnapshot {
                plugins: selected_plugins,
                disabled_plugin_roots,
                disabled_plugin_ids: suppressed_plugin_ids,
            },
        }
    }

    /// Returns config- and plugin-backed servers without runtime contributions.
    pub async fn configured_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        configured_mcp_servers(&mcp_config)
    }

    /// Returns configured and host-contributed servers before auth gating.
    pub async fn runtime_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = self.runtime_config(config).await;
        configured_mcp_servers(&mcp_config)
    }

    /// Returns runtime servers after auth gating and compatibility built-ins.
    pub async fn effective_servers(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> HashMap<String, EffectiveMcpServer> {
        let mcp_config = self.runtime_config(config).await;
        effective_mcp_servers(&mcp_config, auth)
    }
}

fn available_plugin_roots(context: McpServerContributionContext<'_, Config>) -> HashSet<String> {
    let mut roots = context
        .ready_selected_capability_roots()
        .filter(|_| context.executor_capability_discovery_v2().is_none())
        .unwrap_or_default()
        .iter()
        .map(|root| root.id.clone())
        .collect::<HashSet<_>>();
    for snapshot in context
        .executor_capability_discovery_v2()
        .unwrap_or_default()
    {
        if let Ok(inventory) = &snapshot.result {
            roots.extend(
                inventory
                    .plugins
                    .iter()
                    .map(|plugin| format!("{}:{}", snapshot.environment_id, plugin.id)),
            );
        }
    }
    roots
}
