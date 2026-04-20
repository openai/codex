use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::plugins::PluginsManager;
use codex_config::McpServerConfig;
use codex_login::CodexAuth;
use codex_mcp::ToolPluginProvenance;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers_with_authorization_header;
use codex_mcp::tool_plugin_provenance as collect_tool_plugin_provenance;

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self { plugins_manager }
    }

    pub async fn configured_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        configured_mcp_servers(&mcp_config)
    }

    pub async fn effective_servers(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> HashMap<String, McpServerConfig> {
        self.effective_servers_with_authorization_header(
            config, auth, /*authorization_header_value*/ None,
        )
        .await
    }

    pub async fn effective_servers_with_authorization_header(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        authorization_header_value: Option<&str>,
    ) -> HashMap<String, McpServerConfig> {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        effective_mcp_servers_with_authorization_header(
            &mcp_config,
            auth,
            authorization_header_value,
        )
    }

    pub async fn tool_plugin_provenance(&self, config: &Config) -> ToolPluginProvenance {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        collect_tool_plugin_provenance(&mcp_config)
    }
}
