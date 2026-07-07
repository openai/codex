use super::*;
use codex_app_server_protocol::McpToolCallAppContext;
use codex_app_server_protocol::McpToolCallStatus;
use pretty_assertions::assert_eq;

#[test]
fn bounds_entries_and_keeps_updated_origins() {
    let mut index = McpResourceOriginIndex::default();
    insert(&mut index, "call-0", "old-connector");
    for index_value in 1..MAX_MCP_RESOURCE_ORIGINS {
        insert(&mut index, format!("call-{index_value}"), "connector");
    }

    insert(&mut index, "call-0", "new-connector");
    insert(&mut index, "overflow", "connector");

    assert_eq!(
        index.get("call-0"),
        Some(origin_with_app_context("new-connector"))
    );
    assert_eq!(index.get("call-1"), None);
}

#[test]
fn seeds_origins_from_materialized_items() {
    let mut index = McpResourceOriginIndex::default();
    let items = [ThreadItem::McpToolCall {
        id: "call-1".to_string(),
        server: "plugin-runtime".to_string(),
        tool: "search".to_string(),
        status: McpToolCallStatus::Completed,
        arguments: serde_json::Value::Null,
        app_context: Some(McpToolCallAppContext {
            connector_id: "connector".to_string(),
            link_id: Some("link".to_string()),
            resource_uri: Some("ui://shared/widget.html".to_string()),
            app_name: Some("App".to_string()),
            template_id: None,
            action_name: Some("Search".to_string()),
        }),
        mcp_app_resource_uri: Some("ui://shared/widget.html".to_string()),
        plugin_id: None,
        result: None,
        error: None,
        duration_ms: Some(1),
    }];

    index.seed(&items);

    assert_eq!(
        index.get("call-1"),
        Some(origin_with_app_context("connector"))
    );
}

fn insert(index: &mut McpResourceOriginIndex, call_id: impl Into<String>, connector_id: &str) {
    index.insert(call_id.into(), origin_with_app_context(connector_id));
}

fn origin_with_app_context(connector_id: &str) -> McpResourceOrigin {
    McpResourceOrigin {
        server: "plugin-runtime".to_string(),
        tool: "search".to_string(),
        connector_id: connector_id.to_string(),
        link_id: Some("link".to_string()),
        resource_uri: "ui://shared/widget.html".to_string(),
    }
}
