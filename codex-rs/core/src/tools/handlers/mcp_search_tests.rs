use super::*;
use crate::tools::tool_search_entry::ToolSearchEntry;
use codex_tools::JsonSchema;
use codex_tools::LoadableToolSpec;
use codex_tools::ResponsesApiTool;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn search_info_uses_mcp_tool_metadata_and_parameter_names() {
    let handler = McpHandler::new(tool_info());

    assert_eq!(
        handler.search_info().expect("MCP search info").entry,
        ToolSearchEntry {
            search_text: "mcp__calendar___create_event _create_event createEvent codex-apps Create event Create a calendar event. Calendar Plan events. Calendar plugin attendees start_time"
                .to_string(),
            output: LoadableToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Plan events.".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "_create_event".to_string(),
                    description: "Create a calendar event.".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: JsonSchema::object(
                        BTreeMap::from([
                            (
                                "attendees".to_string(),
                                JsonSchema::string(/*description*/ None),
                            ),
                            (
                                "start_time".to_string(),
                                JsonSchema::string(/*description*/ None),
                            ),
                        ]),
                        /*required*/ None,
                        Some(false.into()),
                    ),
                    output_schema: None,
                })],
            }),
        }
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
