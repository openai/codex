//! Plugin-only discovery for install suggestions.
//!
//! Hosted Apps are contributed as ordinary MCP servers by their owning extension, so connector
//! inventory and installation do not pass through the generic tool-discovery path.

use serde::Deserialize;
use serde::Serialize;

const TUI_CLIENT_NAME: &str = "codex-tui";
pub const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub const TOOL_SEARCH_DEFAULT_LIMIT: usize = 8;
pub const LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME: &str = "list_available_plugins_to_install";
pub const REQUEST_PLUGIN_INSTALL_TOOL_NAME: &str = "request_plugin_install";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSearchSourceInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolType {
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolAction {
    Install,
}

pub fn filter_request_plugin_install_candidates_for_client(
    plugins: Vec<DiscoverablePluginInfo>,
    app_server_client_name: Option<&str>,
) -> Vec<DiscoverablePluginInfo> {
    if app_server_client_name == Some(TUI_CLIENT_NAME) {
        Vec::new()
    } else {
        plugins
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoverablePluginInfo {
    pub id: String,
    pub remote_plugin_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tool_type: DiscoverableToolType,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ListAvailablePluginsToInstallResult {
    pub tools: Vec<RequestPluginInstallEntry>,
}

pub fn collect_request_plugin_install_entries(
    plugins: &[DiscoverablePluginInfo],
) -> Vec<RequestPluginInstallEntry> {
    plugins
        .iter()
        .map(|plugin| RequestPluginInstallEntry {
            id: plugin.id.clone(),
            name: plugin.name.clone(),
            description: plugin.description.clone(),
            tool_type: DiscoverableToolType::Plugin,
            has_skills: plugin.has_skills,
            mcp_server_names: plugin.mcp_server_names.clone(),
            app_connector_ids: plugin.app_connector_ids.clone(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_discovery_tests.rs"]
mod tests;
