use super::*;
use codex_tools::LoadableToolSpec;
use codex_tools::ToolSearchSourceInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn search_info_uses_mcp_tool_metadata_and_parameter_names() {
    let handler = McpHandler::new(tool_info());
    let search_info = handler.search_info().expect("MCP search info");

    assert_eq!(
        search_info.entry.search_text,
        "mcp__calendar___create_event _create_event createEvent codex-apps Create event Create a calendar event. Calendar Plan events. Calendar plugin attendees start_time"
    );
    assert_eq!(
        search_info.source_info,
        Some(ToolSearchSourceInfo {
            name: "Calendar".to_string(),
            description: Some("Plan events.".to_string()),
        })
    );
}

#[test]
fn search_info_uses_connector_name_for_output_namespace_description() {
    let mut tool_info = tool_info();
    tool_info.namespace_description = None;
    let handler = McpHandler::new(tool_info);
    let search_info = handler.search_info().expect("MCP search info");

    let LoadableToolSpec::Namespace(namespace) = search_info.entry.output else {
        panic!("expected namespace search output");
    };
    assert_eq!(namespace.description, "Tools for working with Calendar.");
    assert_eq!(
        search_info.source_info,
        Some(ToolSearchSourceInfo {
            name: "Calendar".to_string(),
            description: None,
        })
    );
}

fn tool_info() -> ToolInfo {
    ToolInfo {
        server_name: "codex-apps".to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: "_create_event".to_string(),
        callable_namespace: "mcp__calendar__".to_string(),
        namespace_description: Some("Plan events.".to_string()),
        tool: rmcp::model::Tool {
            name: "createEvent".to_string().into(),
            title: Some("Create event".to_string()),
            description: Some("Create a calendar event.".to_string().into()),
            input_schema: Arc::new(rmcp::model::object(json!({
                "type": "object",
                "properties": {
                    "start_time": { "type": "string" },
                    "attendees": { "type": "string" }
                },
                "additionalProperties": false
            }))),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: None,
        connector_name: Some("Calendar".to_string()),
        plugin_display_names: vec![" Calendar plugin ".to_string(), " ".to_string()],
    }
}
