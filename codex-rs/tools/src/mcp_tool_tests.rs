use super::mcp_call_tool_result_output_schema;
use super::parse_mcp_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use pretty_assertions::assert_eq;

fn mcp_tool(name: &str, description: &str, input_schema: serde_json::Value) -> rmcp::model::Tool {
    rmcp::model::Tool {
        name: name.to_string().into(),
        title: None,
        description: Some(description.to_string().into()),
        input_schema: std::sync::Arc::new(rmcp::model::object(input_schema)),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

#[test]
fn parse_mcp_tool_inserts_empty_properties() {
    let tool = mcp_tool(
        "no_props",
        "No properties",
        serde_json::json!({
            "type": "object"
        }),
    );

    assert_eq!(
        parse_mcp_tool(&tool).expect("parse MCP tool"),
        ToolDefinition {
            name: "no_props".to_string(),
            description: "No properties".to_string(),
            input_schema: JsonSchema::from_raw_tool_input_schema(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: false,
        }
    );
}

#[test]
fn parse_mcp_tool_preserves_raw_input_schema_keywords() {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "parent": {},
            "start": {
                "$ref": "#/$defs/date_time_zone",
                "description": "Event start"
            },
            "end": {
                "allOf": [
                    { "$ref": "#/$defs/date_time_zone" }
                ]
            }
        },
        "$defs": {
            "date_time_zone": {
                "type": "object",
                "properties": {
                    "dateTime": { "type": "string" },
                    "timeZone": { "type": "string" }
                },
                "required": ["dateTime", "timeZone"]
            }
        },
        "additionalProperties": true
    });
    let tool = mcp_tool("create_page", "Create page", input_schema.clone());

    let parsed = parse_mcp_tool(&tool).expect("parse MCP tool");

    assert_eq!(
        serde_json::to_value(&parsed.input_schema).expect("serialize input schema"),
        input_schema
    );
}

#[test]
fn parse_mcp_tool_preserves_top_level_output_schema() {
    let mut tool = mcp_tool(
        "with_output",
        "Has output schema",
        serde_json::json!({
            "type": "object"
        }),
    );
    tool.output_schema = Some(std::sync::Arc::new(rmcp::model::object(
        serde_json::json!({
            "properties": {
                "result": {
                    "properties": {
                        "nested": {}
                    }
                }
            },
            "required": ["result"]
        }),
    )));

    assert_eq!(
        parse_mcp_tool(&tool).expect("parse MCP tool"),
        ToolDefinition {
            name: "with_output".to_string(),
            description: "Has output schema".to_string(),
            input_schema: JsonSchema::from_raw_tool_input_schema(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({
                "properties": {
                    "result": {
                        "properties": {
                            "nested": {}
                        }
                    }
                },
                "required": ["result"]
            }))),
            defer_loading: false,
        }
    );
}

#[test]
fn parse_mcp_tool_preserves_output_schema_without_inferred_type() {
    let mut tool = mcp_tool(
        "with_enum_output",
        "Has enum output schema",
        serde_json::json!({
            "type": "object"
        }),
    );
    tool.output_schema = Some(std::sync::Arc::new(rmcp::model::object(
        serde_json::json!({
            "enum": ["ok", "error"]
        }),
    )));

    assert_eq!(
        parse_mcp_tool(&tool).expect("parse MCP tool"),
        ToolDefinition {
            name: "with_enum_output".to_string(),
            description: "Has enum output schema".to_string(),
            input_schema: JsonSchema::from_raw_tool_input_schema(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({
                "enum": ["ok", "error"]
            }))),
            defer_loading: false,
        }
    );
}
