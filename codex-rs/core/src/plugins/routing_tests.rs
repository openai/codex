use codex_app_server_protocol::AuthMode;
use codex_plugin::AppConnectorId;

use super::*;

fn plugin(app_ids: &[&str], mcp_server_names: &[&str]) -> PluginCapabilitySummary {
    PluginCapabilitySummary {
        config_name: "sample@personal".to_string(),
        display_name: "Sample".to_string(),
        app_connector_ids: app_ids
            .iter()
            .map(|id| AppConnectorId((*id).to_string()))
            .collect(),
        mcp_server_names: mcp_server_names
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        ..PluginCapabilitySummary::default()
    }
}

struct RouteCase {
    name: &'static str,
    plugin: PluginCapabilitySummary,
    auth_mode: Option<AuthMode>,
    expected_plugin_route: PluginRoute,
}

#[test]
fn routes_plugins_by_auth_mode() {
    let cases = vec![
        RouteCase {
            name: "chatgpt dual-surface plugin",
            plugin: plugin(&["connector_sample"], &["sample-mcp"]),
            auth_mode: Some(AuthMode::Chatgpt),
            expected_plugin_route: PluginRoute::Default,
        },
        RouteCase {
            name: "api-key dual-surface plugin",
            plugin: plugin(&["connector_sample"], &["sample-mcp"]),
            auth_mode: Some(AuthMode::ApiKey),
            expected_plugin_route: PluginRoute::McpOnly,
        },
        RouteCase {
            name: "unknown auth dual-surface plugin",
            plugin: plugin(&["connector_sample"], &["sample-mcp"]),
            auth_mode: None,
            expected_plugin_route: PluginRoute::Default,
        },
        RouteCase {
            name: "agent identity dual-surface plugin",
            plugin: plugin(&["connector_sample"], &["sample-mcp"]),
            auth_mode: Some(AuthMode::AgentIdentity),
            expected_plugin_route: PluginRoute::Default,
        },
        RouteCase {
            name: "chatgpt app-only plugin",
            plugin: plugin(&["connector_sample"], &[]),
            auth_mode: Some(AuthMode::Chatgpt),
            expected_plugin_route: PluginRoute::Default,
        },
        RouteCase {
            name: "chatgpt mcp-only plugin",
            plugin: plugin(&[], &["sample-mcp"]),
            auth_mode: Some(AuthMode::Chatgpt),
            expected_plugin_route: PluginRoute::Default,
        },
    ];

    for case in cases {
        assert_eq!(
            route_for_plugin(&case.plugin, case.auth_mode),
            case.expected_plugin_route,
            "{}",
            case.name
        );
    }
}
