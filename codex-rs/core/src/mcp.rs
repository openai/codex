use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::plugins::PluginsManager;
use codex_config::McpServerConfig;
use codex_login::CodexAuth;
use codex_mcp::McpConfig;
use codex_mcp::ToolPluginProvenance;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;
use codex_mcp::tool_plugin_provenance as collect_tool_plugin_provenance;
use codex_model_provider::create_model_provider;

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self { plugins_manager }
    }

    pub async fn mcp_config(&self, config: &Config) -> McpConfig {
        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        let capabilities =
            create_model_provider(config.model_provider.clone(), /*auth_manager*/ None)
                .capabilities();
        if !capabilities.mcp_servers {
            mcp_config.configured_mcp_servers.clear();
        }
        if !capabilities.app_connectors {
            mcp_config.apps_enabled = false;
        }
        mcp_config
    }

    pub async fn configured_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = self.mcp_config(config).await;
        configured_mcp_servers(&mcp_config)
    }

    pub async fn effective_servers(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> HashMap<String, McpServerConfig> {
        let mcp_config = self.mcp_config(config).await;
        effective_mcp_servers(&mcp_config, auth)
    }

    pub async fn tool_plugin_provenance(&self, config: &Config) -> ToolPluginProvenance {
        let mcp_config = self.mcp_config(config).await;
        collect_tool_plugin_provenance(&mcp_config)
    }
}
