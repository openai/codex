use std::collections::HashSet;
use std::sync::Arc;

use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;

use super::*;
use crate::config::test_config;
use crate::connectors::AppInfo;

fn make_connector(id: &str, name: &str) -> AppInfo {
    AppInfo {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: true,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }
}

fn make_mcp_tool(
    server_name: &str,
    tool_name: &str,
    callable_namespace: &str,
    callable_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: callable_name.to_string(),
        callable_namespace: callable_namespace.to_string(),
        namespace_description: None,
        tool: Tool::new(
            tool_name.to_string(),
            format!("Test tool: {tool_name}"),
            Arc::new(JsonObject::default()),
        ),
        connector_id: connector_id.map(str::to_string),
        connector_name: connector_name.map(str::to_string),
        plugin_display_names: Vec::new(),
    }
}

fn numbered_mcp_tools(count: usize) -> Vec<ToolInfo> {
    (0..count)
        .map(|index| {
            let tool_name = format!("tool_{index}");
            make_mcp_tool(
                "rmcp",
                &tool_name,
                "mcp__rmcp",
                &tool_name,
                /*connector_id*/ None,
                /*connector_name*/ None,
            )
        })
        .collect()
}

fn tool_names(tools: &[ToolInfo]) -> HashSet<ToolName> {
    tools
        .iter()
        .map(codex_mcp::ToolInfo::canonical_tool_name)
        .collect()
}

#[tokio::test]
async fn directly_exposes_small_effective_tool_sets() {
    let config = test_config().await;
    let mcp_tools = numbered_mcp_tools(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD - 1);

    let exposure = build_mcp_tool_exposure(
        &mcp_tools, /*connectors*/ None, &config, /*search_tool_enabled*/ true,
    );

    assert_eq!(tool_names(&exposure.direct_tools), tool_names(&mcp_tools));
    assert!(exposure.deferred_tools.is_none());
}

#[tokio::test]
async fn searches_large_effective_tool_sets() {
    let config = test_config().await;
    let mcp_tools = numbered_mcp_tools(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD);

    let exposure = build_mcp_tool_exposure(
        &mcp_tools, /*connectors*/ None, &config, /*search_tool_enabled*/ true,
    );

    assert!(exposure.direct_tools.is_empty());
    let deferred_tools = exposure
        .deferred_tools
        .as_ref()
        .expect("large tool sets should be discoverable through tool_search");
    assert_eq!(tool_names(deferred_tools), tool_names(&mcp_tools));
}

#[tokio::test]
async fn always_defer_feature_defers_apps_too() {
    let mut config = test_config().await;
    config
        .features
        .enable(Feature::ToolSearchAlwaysDeferMcpTools)
        .expect("test config should allow feature update");
    let mcp_tools = vec![
        make_mcp_tool(
            "rmcp",
            "tool",
            "mcp__rmcp",
            "tool",
            /*connector_id*/ None,
            /*connector_name*/ None,
        ),
        make_mcp_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_create_event",
            "mcp__codex_apps__calendar",
            "_create_event",
            Some("calendar"),
            Some("Calendar"),
        ),
    ];
    let connectors = vec![make_connector("calendar", "Calendar")];

    let exposure = build_mcp_tool_exposure(
        &mcp_tools,
        Some(connectors.as_slice()),
        &config,
        /*search_tool_enabled*/ true,
    );

    assert!(exposure.direct_tools.is_empty());
    let deferred_tools = exposure
        .deferred_tools
        .as_ref()
        .expect("MCP tools should be discoverable through tool_search");
    let deferred_tool_names = tool_names(deferred_tools);
    assert!(deferred_tool_names.contains(&ToolName::namespaced("mcp__rmcp", "tool")));
    assert!(deferred_tool_names.contains(&ToolName::namespaced(
        "mcp__codex_apps__calendar",
        "_create_event"
    )));
}
