#![allow(dead_code)]

use codex_app_server_protocol::AuthMode;

use crate::plugins::PluginCapabilitySummary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PluginRoute {
    Default,
    McpOnly,
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

fn is_dual_surface_plugin(plugin: &PluginCapabilitySummary) -> bool {
    !plugin.app_connector_ids.is_empty() && !plugin.mcp_server_names.is_empty()
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
