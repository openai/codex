use std::collections::HashSet;
use std::sync::Arc;

use codex_app_server_protocol::AuthMode;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use codex_plugin::AppConnectorId;
use rmcp::model::JsonObject;
use rmcp::model::Tool;

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

struct ToolFilterCase {
    name: &'static str,
    tools: Vec<ToolInfo>,
    plugins: Vec<PluginCapabilitySummary>,
    auth_mode: Option<AuthMode>,
    expected_callable_names: &'static [&'static str],
}

fn mcp_tool(server_name: &str, callable_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: callable_name.to_string(),
        callable_namespace: format!("mcp__{server_name}"),
        namespace_description: None,
        tool: Tool::new(
            callable_name.to_string(),
            format!("Test tool: {callable_name}"),
            Arc::new(JsonObject::default()),
        ),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn app_tool(connector_id: &str, callable_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: callable_name.to_string(),
        callable_namespace: format!("mcp__codex_apps__{connector_id}"),
        namespace_description: None,
        tool: Tool::new(
            callable_name.to_string(),
            format!("Test tool: {callable_name}"),
            Arc::new(JsonObject::default()),
        ),
        connector_id: Some(connector_id.to_string()),
        connector_name: Some(connector_id.to_string()),
        plugin_display_names: Vec::new(),
    }
}

fn callable_names(tools: &[ToolInfo]) -> HashSet<&str> {
    tools
        .iter()
        .map(|tool| tool.callable_name.as_str())
        .collect()
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

#[test]
fn filters_model_visible_tools_by_auth_route() {
    let cases = vec![
        ToolFilterCase {
            name: "chatgpt dual-surface plugin exposes app and mcp tools",
            tools: vec![
                mcp_tool("sample-mcp", "mcp_search"),
                app_tool("connector_sample", "app_search"),
            ],
            plugins: vec![plugin(&["connector_sample"], &["sample-mcp"])],
            auth_mode: Some(AuthMode::Chatgpt),
            expected_callable_names: &["app_search", "mcp_search"],
        },
        ToolFilterCase {
            name: "api-key dual-surface plugin exposes mcp tools",
            tools: vec![
                mcp_tool("sample-mcp", "mcp_search"),
                app_tool("connector_sample", "app_search"),
            ],
            plugins: vec![plugin(&["connector_sample"], &["sample-mcp"])],
            auth_mode: Some(AuthMode::ApiKey),
            expected_callable_names: &["mcp_search"],
        },
        ToolFilterCase {
            name: "chatgpt dual-surface plugin with only mcp tool exposes mcp tool",
            tools: vec![mcp_tool("sample-mcp", "mcp_search")],
            plugins: vec![plugin(&["connector_sample"], &["sample-mcp"])],
            auth_mode: Some(AuthMode::Chatgpt),
            expected_callable_names: &["mcp_search"],
        },
        ToolFilterCase {
            name: "single-surface and unowned tools remain visible",
            tools: vec![
                mcp_tool("sample-mcp", "mcp_search"),
                app_tool("connector_sample", "app_search"),
                mcp_tool("user-server", "user_search"),
            ],
            plugins: vec![plugin(&[], &["sample-mcp"])],
            auth_mode: Some(AuthMode::Chatgpt),
            expected_callable_names: &["mcp_search", "app_search", "user_search"],
        },
        ToolFilterCase {
            name: "shared mcp server remains visible if any owner routes to mcp",
            tools: vec![mcp_tool("shared-mcp", "shared_search")],
            plugins: vec![
                plugin(&["connector_sample"], &["shared-mcp"]),
                plugin(&[], &["shared-mcp"]),
            ],
            auth_mode: Some(AuthMode::Chatgpt),
            expected_callable_names: &["shared_search"],
        },
    ];

    for case in cases {
        let filtered = filter_model_visible_tools_for_plugin_routes(
            &case.tools,
            &case.plugins,
            case.auth_mode,
        );
        let expected_callable_names = case
            .expected_callable_names
            .iter()
            .copied()
            .collect::<HashSet<_>>();

        assert_eq!(
            callable_names(&filtered),
            expected_callable_names,
            "{}",
            case.name
        );
    }
}
