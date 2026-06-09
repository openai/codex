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
use codex_mcp::ToolPluginProvenance;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;
use codex_mcp::tool_plugin_provenance as collect_tool_plugin_provenance;

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

    /// Returns the MCP config after applying runtime-only extension overlays.
    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        let contributions = self.contributions(config).await;
        if contributions
            .iter()
            .any(|contribution| contribution.name() == CODEX_APPS_MCP_SERVER_NAME)
        {
            mcp_config.legacy_apps_mcp_loader_enabled = false;
        }
        Self::apply_to_configured_servers(&contributions, &mut mcp_config.configured_mcp_servers);
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

    /// Returns provenance for plugin-owned servers in the configured view.
    pub async fn tool_plugin_provenance(&self, config: &Config) -> ToolPluginProvenance {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        collect_tool_plugin_provenance(&mcp_config)
    }

    async fn contributions(&self, config: &Config) -> Vec<McpServerContribution> {
        let mut contributions = Vec::new();
        for contributor in self.extensions.mcp_server_contributors() {
            contributions.extend(contributor.contribute(config).await);
        }
        contributions
    }

    fn apply_to_configured_servers(
        contributions: &[McpServerContribution],
        servers: &mut HashMap<String, McpServerConfig>,
    ) {
        for contribution in contributions {
            match contribution {
                McpServerContribution::Set { name, config } => {
                    servers.insert(name.clone(), config.as_ref().clone());
                }
                McpServerContribution::Remove { name } => {
                    servers.remove(name);
                }
            }
        }
    }
}
