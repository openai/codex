use std::sync::Arc;

use codex_mcp::ToolInfo;
use rmcp::model::JsonObject;
use rmcp::model::Tool;

use super::*;

fn tool(name: &str) -> ToolInfo {
    ToolInfo {
        server_name: "server".to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: name.to_string(),
        callable_namespace: "mcp__server".to_string(),
        namespace_description: None,
        namespace_title: None,
        search_aliases: Vec::new(),
        tool: Tool::new(
            name.to_string(),
            format!("Test tool: {name}"),
            Arc::new(JsonObject::default()),
        ),
        plugin_display_names: Vec::new(),
    }
}

#[test]
fn exposes_tools_directly_without_search() {
    let tools = vec![tool("one"), tool("two")];
    let exposure = build_mcp_tool_exposure(&tools, /*search_tool_enabled*/ false);

    assert_eq!(
        exposure
            .direct_tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
    assert!(exposure.deferred_tools.is_none());
}

#[test]
fn defers_tools_when_search_is_enabled() {
    let tools = vec![tool("one"), tool("two")];
    let exposure = build_mcp_tool_exposure(&tools, /*search_tool_enabled*/ true);

    assert!(exposure.direct_tools.is_empty());
    assert_eq!(
        exposure
            .deferred_tools
            .expect("deferred tools")
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "two"]
    );
}
