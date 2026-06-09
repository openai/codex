#![allow(dead_code)]

use codex_app_server_protocol::AuthMode;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;

use crate::plugins::PluginCapabilitySummary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PluginRoute {
    Default,
    McpOnly,
}

impl PluginRoute {
    fn allows_app_tools(self) -> bool {
        matches!(self, Self::Default)
    }

    fn allows_mcp_tools(self) -> bool {
        matches!(self, Self::Default | Self::McpOnly)
    }
}

pub(crate) fn route_for_plugin(
    plugin: &PluginCapabilitySummary,
    auth_mode: Option<AuthMode>,
) -> PluginRoute {
    if !is_dual_surface_plugin(plugin) {
        return PluginRoute::Default;
    }

    if auth_mode == Some(AuthMode::ApiKey) {
        return PluginRoute::McpOnly;
    }

    PluginRoute::Default
}

pub(crate) fn filter_model_visible_tools_for_plugin_routes(
    tools: &[ToolInfo],
    plugins: &[PluginCapabilitySummary],
    auth_mode: Option<AuthMode>,
) -> Vec<ToolInfo> {
    let plugin_routes = plugins
        .iter()
        .map(|plugin| (plugin, route_for_plugin(plugin, auth_mode)))
        .collect::<Vec<_>>();

    tools
        .iter()
        .filter(|tool| tool_allowed_for_plugin_routes(tool, &plugin_routes))
        .cloned()
        .collect()
}

fn tool_allowed_for_plugin_routes(
    tool: &ToolInfo,
    plugin_routes: &[(&PluginCapabilitySummary, PluginRoute)],
) -> bool {
    if tool.server_name == CODEX_APPS_MCP_SERVER_NAME {
        let Some(connector_id) = tool.connector_id.as_deref() else {
            return true;
        };
        return connector_tool_allowed_for_plugin_routes(connector_id, plugin_routes);
    }

    mcp_tool_allowed_for_plugin_routes(tool.server_name.as_str(), plugin_routes)
}

fn connector_tool_allowed_for_plugin_routes(
    connector_id: &str,
    plugin_routes: &[(&PluginCapabilitySummary, PluginRoute)],
) -> bool {
    let mut owned_by_plugin = false;

    for (plugin, route) in plugin_routes {
        if !plugin
            .app_connector_ids
            .iter()
            .any(|plugin_connector_id| plugin_connector_id.0 == connector_id)
        {
            continue;
        }
        owned_by_plugin = true;
        if route.allows_app_tools() {
            return true;
        }
    }

    !owned_by_plugin
}

fn mcp_tool_allowed_for_plugin_routes(
    server_name: &str,
    plugin_routes: &[(&PluginCapabilitySummary, PluginRoute)],
) -> bool {
    let mut owned_by_plugin = false;

    for (plugin, route) in plugin_routes {
        if !plugin
            .mcp_server_names
            .iter()
            .any(|plugin_server_name| plugin_server_name == server_name)
        {
            continue;
        }
        owned_by_plugin = true;
        if route.allows_mcp_tools() {
            return true;
        }
    }

    !owned_by_plugin
}

fn is_dual_surface_plugin(plugin: &PluginCapabilitySummary) -> bool {
    !plugin.app_connector_ids.is_empty() && !plugin.mcp_server_names.is_empty()
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
