use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;

use crate::connectors;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::plugins::PluginCapabilitySummary;
use crate::plugins::render_explicit_plugin_instructions;

pub(crate) fn build_plugin_injections(
    mentioned_plugins: &[PluginCapabilitySummary],
    mcp_tools: &HashMap<String, ToolInfo>,
    available_connectors: &[connectors::AppInfo],
) -> Vec<ResponseItem> {
    if mentioned_plugins.is_empty() {
        return Vec::new();
    }

    let visible_mcp_server_names = mcp_tools
        .values()
        .filter(|tool| tool.server_name != CODEX_APPS_MCP_SERVER_NAME)
        .map(|tool| tool.server_name.clone())
        .collect::<HashSet<String>>();
    let enabled_connectors_by_id = available_connectors
        .iter()
        .filter(|connector| connector.is_enabled)
        .map(|connector| {
            (
                connector.id.as_str(),
                connectors::connector_display_label(connector),
            )
        })
        .collect::<HashMap<&str, String>>();

    // Turn each explicit @plugin mention into a developer hint that points the
    // model at the plugin's visible MCP servers, enabled apps, and skill prefix.
    mentioned_plugins
        .iter()
        .filter_map(|plugin| {
            let available_mcp_servers = plugin
                .mcp_server_names
                .iter()
                .filter(|server_name| visible_mcp_server_names.contains(server_name.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let available_apps = plugin
                .app_connector_ids
                .iter()
                .filter_map(|connector_id| enabled_connectors_by_id.get(connector_id.0.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            render_explicit_plugin_instructions(plugin, &available_mcp_servers, &available_apps)
                .map(DeveloperInstructions::new)
                .map(ResponseItem::from)
        })
        .collect()
}
