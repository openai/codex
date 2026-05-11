use super::parse_dynamic_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use pretty_assertions::assert_eq;

#[test]
fn parse_dynamic_tool_preserves_raw_input_schema() {
    let tool = DynamicToolSpec {
        namespace: None,
        name: "lookup_ticket".to_string(),
        description: "Fetch a ticket".to_string(),
        input_schema: serde_json::json!({
            "properties": {
                "id": {
                    "description": "Ticket identifier"
                }
            }
        }),
        defer_loading: false,
    };

    assert_eq!(
        parse_dynamic_tool(&tool).expect("parse dynamic tool"),
        ToolDefinition {
            name: "lookup_ticket".to_string(),
            description: "Fetch a ticket".to_string(),
            input_schema: JsonSchema::from_raw_tool_input_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "description": "Ticket identifier"
                    }
                }
            })),
            output_schema: None,
            defer_loading: false,
        }
    );
}

#[test]
fn parse_dynamic_tool_preserves_defer_loading() {
    let tool = DynamicToolSpec {
        namespace: None,
        name: "lookup_ticket".to_string(),
        description: "Fetch a ticket".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        defer_loading: true,
    };

    assert_eq!(
        parse_dynamic_tool(&tool).expect("parse dynamic tool"),
        ToolDefinition {
            name: "lookup_ticket".to_string(),
            description: "Fetch a ticket".to_string(),
            input_schema: JsonSchema::from_raw_tool_input_schema(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
            output_schema: None,
            defer_loading: true,
        }
    );
}
