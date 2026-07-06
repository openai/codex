use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use codex_config::McpServerConfig;
use codex_connectors::ConnectorSnapshot;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::PluginsManager;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::CodexAppsToolsCache;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::McpPluginAttribution;
use codex_mcp::McpServerRegistration;
use codex_mcp::codex_apps_mcp_server_config;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;
use codex_plugin::AppConnectorId;

const LEGACY_CODEX_APPS_REGISTRATION_ID: &str = "legacy_codex_apps";

/// MCP configuration and capability availability derived from the same inputs.
pub(crate) struct McpRuntimeProjection {
    pub(crate) config: McpConfig,
    pub(crate) plugins_available: bool,
}

enum OrderedMcpOverlay {
    Set {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
        config: Box<McpServerConfig>,
    },
    Remove {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
    },
}

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
    extensions: Arc<ExtensionRegistry<Config>>,
    codex_apps_tools_cache: CodexAppsToolsCache,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self::new_with_extensions(
            plugins_manager,
            codex_extension_api::empty_extension_registry(),
        )
    }

    /// Creates a manager that resolves host-installed MCP contributions.
    pub fn new_with_extensions(
        plugins_manager: Arc<PluginsManager>,
        extensions: Arc<ExtensionRegistry<Config>>,
    ) -> Self {
        Self {
            plugins_manager,
            extensions,
            codex_apps_tools_cache: CodexAppsToolsCache::default(),
        }
    }

    pub fn codex_apps_tools_cache(&self) -> CodexAppsToolsCache {
        self.codex_apps_tools_cache.clone()
    }

    /// Returns the MCP config after applying compatibility built-ins and
    /// runtime-only extension overlays.
    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        self.runtime_config_with_context(McpServerContributionContext::global(config))
            .await
            .config
    }

    pub(crate) async fn runtime_config_for_step(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        available_environment_ids: &[String],
    ) -> McpRuntimeProjection {
        self.runtime_config_with_context(McpServerContributionContext::for_step(
            config,
            thread_init,
            thread_store,
            available_environment_ids,
        ))
        .await
    }

    async fn runtime_config_with_context(
        &self,
        context: McpServerContributionContext<'_, Config>,
    ) -> McpRuntimeProjection {
        let config = context.config();
        let mut selected_plugin_available = false;
        let mut selected_plugin_connector_sources = Vec::new();
        let mut selected_plugin_registrations = Vec::new();
        let mut overlays = Vec::new();
        // A contributor can emit multiple ordered actions, so order each action globally rather
        // than enumerating contributors.
        let mut contribution_order = 0;
        for contributor in self.extensions.mcp_server_contributors() {
            for contribution in contributor.contribute(context).await {
                match contribution {
                    McpServerContribution::Set { name, config } => {
                        overlays.push(OrderedMcpOverlay::Set {
                            contributor_id: contributor.id(),
                            contribution_order,
                            name,
                            config,
                        });
                    }
                    McpServerContribution::SelectedPlugin {
                        name,
                        plugin_id,
                        plugin_display_name,
                        selection_order,
                        config,
                    } => selected_plugin_registrations.push(
                        McpServerRegistration::from_selected_plugin(
                            name,
                            McpPluginAttribution::new(plugin_id, plugin_display_name),
                            selection_order,
                            *config,
                        ),
                    ),
                    McpServerContribution::SelectedPluginPackage {
                        plugin_id,
                        plugin_display_name,
                        connector_ids,
                    } => {
                        selected_plugin_available = true;
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
        let plugins_available =
            selected_plugin_available || !loaded_plugins.capability_summaries().is_empty();
        let mut mcp_config = config
            .to_mcp_config_with_loaded_plugins(&loaded_plugins, selected_plugin_registrations);
        let mut catalog = mcp_config.mcp_server_catalog.to_builder();
        if mcp_config.apps_enabled {
            catalog.register(McpServerRegistration::from_compatibility(
                CODEX_APPS_MCP_SERVER_NAME.to_string(),
                LEGACY_CODEX_APPS_REGISTRATION_ID,
                codex_apps_mcp_server_config(
                    &mcp_config.chatgpt_base_url,
                    mcp_config.apps_mcp_product_sku.as_deref(),
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
                OrderedMcpOverlay::Set {
                    contributor_id,
                    contribution_order,
                    name,
                    config,
                } => catalog.register(McpServerRegistration::from_extension(
                    name,
                    contributor_id,
                    contribution_order,
                    *config,
                )),
                OrderedMcpOverlay::Remove {
                    contributor_id,
                    contribution_order,
                    name,
                } => catalog.remove_extension(name, contributor_id, contribution_order),
            }
        }
        let catalog = catalog.build();
        for conflict in catalog.conflicts() {
            tracing::warn!(
                server = conflict.name,
                outcome = ?conflict.outcome,
                contenders = ?conflict.contenders,
                "conflicting MCP server actions; using resolved catalog outcome"
            );
        }
        mcp_config.mcp_server_catalog = catalog;
        mcp_config.connector_snapshot =
            mcp_config
                .connector_snapshot
                .merged_with(&ConnectorSnapshot::from_plugin_sources(
                    selected_plugin_connector_sources,
                ));
        McpRuntimeProjection {
            config: mcp_config,
            plugins_available,
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
