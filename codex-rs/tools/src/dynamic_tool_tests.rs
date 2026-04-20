use super::parse_dynamic_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn parse_dynamic_tool_sanitizes_input_schema() {
    let tool = DynamicToolSpec {
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
            name: ToolName::plain("lookup_ticket"),
            description: "Fetch a ticket".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::from([(
                    "id".to_string(),
                    JsonSchema::string(Some("Ticket identifier".to_string()),),
                )]),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: None,
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Dynamic,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
        }
    );
}

#[test]
fn parse_dynamic_tool_preserves_defer_loading() {
    let tool = DynamicToolSpec {
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
            name: ToolName::plain("lookup_ticket"),
            description: "Fetch a ticket".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: None,
            loading: ToolLoadingPolicy::Deferred,
            execution: ToolExecution::Dynamic,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
        }
    );
}
