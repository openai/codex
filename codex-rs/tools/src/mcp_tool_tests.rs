use super::ParsedMcpTool;
use super::mcp_call_tool_result_output_schema;
use super::parse_mcp_tool;
use crate::JsonSchema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

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
        ParsedMcpTool {
            description: "No properties".to_string(),
            input_schema: JsonSchema::Object {
                properties: BTreeMap::new(),
                required: None,
                additional_properties: None,
            },
            output_schema: mcp_call_tool_result_output_schema(serde_json::json!({})),
        }
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
        ParsedMcpTool {
            description: "Has output schema".to_string(),
            input_schema: JsonSchema::Object {
                properties: BTreeMap::new(),
                required: None,
                additional_properties: None,
            },
            output_schema: mcp_call_tool_result_output_schema(serde_json::json!({
                "properties": {
                    "result": {
                        "properties": {
                            "nested": {}
                        }
                    }
                },
                "required": ["result"]
            })),
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
        ParsedMcpTool {
            description: "Has enum output schema".to_string(),
            input_schema: JsonSchema::Object {
                properties: BTreeMap::new(),
                required: None,
                additional_properties: None,
            },
            output_schema: mcp_call_tool_result_output_schema(serde_json::json!({
                "enum": ["ok", "error"]
            })),
        }
    );
}
