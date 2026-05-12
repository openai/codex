use super::parse_dynamic_tool;
use crate::ToolDefinition;
use crate::ToolExposure;
use crate::ToolName;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::FunctionToolSpec;
use pretty_assertions::assert_eq;

#[test]
fn parse_dynamic_tool_preserves_definition_metadata() {
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
        parse_dynamic_tool(&tool),
        ToolDefinition::new(
            ToolName::plain("lookup_ticket"),
            FunctionToolSpec {
                name: "lookup_ticket".to_string(),
                description: "Fetch a ticket".to_string(),
                strict: false,
                parameters: serde_json::json!({
                    "properties": {
                        "id": {
                            "description": "Ticket identifier"
                        }
                    }
                }),
            },
            (),
        )
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

    let definition = parse_dynamic_tool(&tool);

    assert_eq!(definition.exposure(), ToolExposure::Deferred);
    assert_eq!(definition.tool_name(), &ToolName::plain("lookup_ticket"));
}
