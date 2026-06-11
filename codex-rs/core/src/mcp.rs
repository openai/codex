use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use codex_config::McpServerConfig;
use codex_core_plugins::PluginsManager;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::McpServerContribution;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::McpServerRegistration;
use codex_mcp::codex_apps_mcp_server_config;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;

const LEGACY_CODEX_APPS_REGISTRATION_ID: &str = "legacy_codex_apps";

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
        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        let disabled_server_names = mcp_config
            .mcp_server_catalog
            .disabled_server_names()
            .map(str::to_string)
            .collect::<Vec<_>>();
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

        let mut contribution_order = 0;
        for contributor in self.extensions.mcp_server_contributors() {
            for contribution in contributor.contribute(config).await {
                match contribution {
                    McpServerContribution::Set {
                        name,
                        config: server_config,
                    } => catalog.register(McpServerRegistration::from_extension(
                        name,
                        contributor.id(),
                        contribution_order,
                        *server_config,
                    )),
                    McpServerContribution::Remove { name } => {
                        catalog.remove_extension(name, contributor.id(), contribution_order)
                    }
                }
                contribution_order += 1;
            }
        }
        for name in disabled_server_names {
            catalog.disable(name);
        }
        let catalog = catalog.build();
        for conflict in catalog.conflicts() {
            tracing::warn!(
                server = conflict.name,
                winner = ?conflict.winner,
                shadowed = ?conflict.shadowed,
                "conflicting MCP server registrations; using explicit source precedence"
            );
        }
        mcp_config.mcp_server_catalog = catalog;
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
