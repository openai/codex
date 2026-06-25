use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use codex_config::McpServerConfig;
use codex_connectors::ConnectorSnapshot;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::ExecutorPluginRuntime;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::apps_route_available;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::McpPluginAttribution;
use codex_mcp::McpServerRegistration;
use codex_mcp::codex_apps_mcp_server_config;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;

const LEGACY_CODEX_APPS_REGISTRATION_ID: &str = "legacy_codex_apps";

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
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self {
            plugins_manager,
            extensions: codex_extension_api::empty_extension_registry(),
        }
    }

    /// Creates a manager that resolves host-installed MCP contributions.
    pub fn new_with_extensions(
        plugins_manager: Arc<PluginsManager>,
        extensions: Arc<ExtensionRegistry<Config>>,
    ) -> Self {
        Self {
            plugins_manager,
            extensions,
        }
    }

    /// Returns the MCP config after applying compatibility built-ins and
    /// runtime-only extension overlays.
    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        let context = McpServerContributionContext::global(config);
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

        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
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
        mcp_config
    }

    /// Adds capabilities read from the executor bindings captured for one model step.
    pub(crate) fn runtime_config_for_executor_plugins(
        &self,
        base_config: &McpConfig,
        config: &Config,
        plugins: &[(usize, ExecutorPluginRuntime)],
    ) -> McpConfig {
        // Step runtimes start from the clean global config, never the thread bootstrap view.
        let mut mcp_config = base_config.clone();
        let mut catalog = mcp_config.mcp_server_catalog.to_builder();
        let selected_connector_snapshot =
            ConnectorSnapshot::from_plugin_sources(plugins.iter().map(|(_, runtime)| {
                let plugin = runtime.plugin();
                PluginConnectorSource::new(
                    plugin.manifest().display_name(),
                    runtime.apps().iter().cloned(),
                )
            }));
        let route_apps_over_mcp = config.orchestrator_mcp_enabled
            && config
                .features
                .apps_enabled_for_auth(apps_route_available(self.plugins_manager.auth_mode()));

        for (selection_order, runtime) in plugins {
            let plugin = runtime.plugin();
            let plugin_id = plugin.selected_root_id().to_string();
            let display_name = plugin.manifest().display_name().to_string();
            let mut servers = runtime
                .mcp_servers()
                .iter()
                .cloned()
                .collect::<HashMap<_, _>>();
            config.apply_plugin_mcp_server_requirements(&plugin_id, &mut servers);
            let mut servers = servers.into_iter().collect::<Vec<_>>();
            servers.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            let attribution = McpPluginAttribution::new(plugin_id.clone(), display_name.clone());
            for (name, server) in servers {
                let has_app = runtime
                    .apps()
                    .iter()
                    .any(|app| app.name == name && !app.connector_id.0.trim().is_empty());
                if !route_apps_over_mcp || !has_app {
                    catalog.register(McpServerRegistration::from_selected_plugin(
                        name,
                        attribution.clone(),
                        *selection_order,
                        server,
                    ));
                }
            }
        }

        let catalog = catalog.build();
        for conflict in catalog.conflicts() {
            tracing::warn!(
                server = conflict.name,
                outcome = ?conflict.outcome,
                contenders = ?conflict.contenders,
                "conflicting selected MCP server actions; using resolved catalog outcome"
            );
        }
        mcp_config.mcp_server_catalog = catalog;
        mcp_config.connector_snapshot = mcp_config
            .connector_snapshot
            .merged_with(&selected_connector_snapshot);
        mcp_config
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
