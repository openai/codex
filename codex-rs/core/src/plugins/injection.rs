use std::collections::BTreeSet;
use std::collections::HashMap;

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

    // Turn each explicit plugin mention into a developer hint that points the
    // model at the plugin's visible MCP servers, enabled apps, and skill prefix.
    mentioned_plugins
        .iter()
        .filter_map(|plugin| {
            let available_mcp_servers = mcp_tools
                .values()
                .filter(|tool| {
                    tool.server_name != CODEX_APPS_MCP_SERVER_NAME
                        && tool
                            .plugin_display_names
                            .iter()
                            .any(|plugin_name| plugin_name == &plugin.display_name)
                })
                .map(|tool| tool.server_name.clone())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect::<Vec<_>>();
            let available_apps = available_connectors
                .iter()
                .filter(|connector| {
                    connector.is_enabled
                        && connector
                            .plugin_display_names
                            .iter()
                            .any(|plugin_name| plugin_name == &plugin.display_name)
                })
                .map(connectors::connector_display_label)
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect::<Vec<_>>();
            render_explicit_plugin_instructions(plugin, &available_mcp_servers, &available_apps)
                .map(DeveloperInstructions::new)
                .map(ResponseItem::from)
        })
        .collect()
}
