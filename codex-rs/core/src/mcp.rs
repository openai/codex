use std::collections::HashMap;
use std::sync::Arc;

pub use codex_mcp::mcp::CODEX_APPS_MCP_SERVER_NAME;
pub use codex_mcp::mcp::McpConfig;
pub use codex_mcp::mcp::ToolPluginProvenance;
pub use codex_mcp::mcp::auth;
pub use codex_mcp::mcp::canonical_mcp_server_key;
pub use codex_mcp::mcp::collect_mcp_snapshot;
pub use codex_mcp::mcp::collect_mcp_snapshot_from_manager;
pub use codex_mcp::mcp::collect_missing_mcp_dependencies;
pub use codex_mcp::mcp::configured_mcp_servers;
pub use codex_mcp::mcp::effective_mcp_servers;
pub use codex_mcp::mcp::group_tools_by_server;
pub use codex_mcp::mcp::qualified_mcp_tool_name_prefix;
pub use codex_mcp::mcp::split_qualified_tool_name;
pub use codex_mcp::mcp::tool_plugin_provenance as mcp_tool_plugin_provenance;
pub use codex_mcp::mcp::with_codex_apps_mcp;

use crate::CodexAuth;
use crate::config::Config;
use crate::plugins::PluginsManager;
use codex_config::McpServerConfig;

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self { plugins_manager }
    }

    pub fn configured_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = config.to_mcp_config(/*auth*/ None, self.plugins_manager.as_ref());
        configured_mcp_servers(&mcp_config)
    }

    pub fn effective_servers(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> HashMap<String, McpServerConfig> {
        let mcp_config = config.to_mcp_config(auth, self.plugins_manager.as_ref());
        effective_mcp_servers(&mcp_config, auth)
    }

    pub fn tool_plugin_provenance(&self, config: &Config) -> ToolPluginProvenance {
        let mcp_config = config.to_mcp_config(/*auth*/ None, self.plugins_manager.as_ref());
        mcp_tool_plugin_provenance(&mcp_config)
    }
}
