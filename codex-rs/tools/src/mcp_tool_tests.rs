use super::mcp_call_tool_result_output_schema;
use super::parse_agent_plugin_mcp_tool;
use super::parse_mcp_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn mcp_tool(name: &str, description: &str, input_schema: serde_json::Value) -> rmcp::model::Tool {
    rmcp::model::Tool::new(
        name.to_string(),
        description.to_string(),
        std::sync::Arc::new(rmcp::model::object(input_schema)),
    )
}

#[test]
fn mcp_schema_max_bytes_uses_the_utf8_compaction_threshold() {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {"query": {"type": "string", "description": "parameter guidance é ".repeat(400)}},
        "required": ["query"],
        "additionalProperties": false,
    });
    let tool = mcp_tool("search", "Search", input_schema.clone());
    let schema_bytes = serde_json::to_vec(&input_schema)
        .expect("schema JSON")
        .len();
    let compacted = super::parse_mcp_tool_with_schema_max_bytes(&tool, schema_bytes - 1)
        .expect("argument types fit without descriptions");
    let expanded = super::parse_mcp_tool_with_schema_max_bytes(&tool, schema_bytes)
        .expect("parameter description fits");
    let mut expected_compacted = input_schema.clone();
    expected_compacted["properties"]["query"] = serde_json::json!({"type": "string"});
    assert_eq!(
        serde_json::to_value(compacted.input_schema).expect("schema"),
        expected_compacted
    );
    assert_eq!(
        serde_json::to_value(expanded.input_schema).expect("schema"),
        input_schema
    );
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
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({})).into()),
            defer_loading: false,
        }
    );
}

#[test]
fn agent_plugin_mcp_tool_bounds_model_visible_description() {
    let description = format!("{}é", "a".repeat(super::MAX_MCP_TOOL_DESCRIPTION_BYTES - 1));
    let tool = mcp_tool(
        "bounded_description",
        &description,
        serde_json::json!({"type": "object"}),
    );

    let parsed = parse_agent_plugin_mcp_tool(&tool).expect("parse Agent Plugin MCP tool");

    assert_eq!(
        parsed.description.len(),
        super::MAX_MCP_TOOL_DESCRIPTION_BYTES - 1
    );
}

#[test]
fn legacy_mcp_tool_preserves_long_description() {
    let description = "a".repeat(super::MAX_MCP_TOOL_DESCRIPTION_BYTES + 100);
    let tool = mcp_tool(
        "legacy_description",
        &description,
        serde_json::json!({"type": "object"}),
    );

    assert_eq!(
        parse_mcp_tool(&tool)
            .expect("parse legacy MCP tool")
            .description,
        description
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
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(
                mcp_call_tool_result_output_schema(serde_json::json!({
                    "properties": {
                        "result": {
                            "properties": {
                                "nested": {}
                            }
                        }
                    },
                    "required": ["result"]
                }))
                .into()
            ),
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
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(
                mcp_call_tool_result_output_schema(serde_json::json!({
                    "enum": ["ok", "error"]
                }))
                .into()
            ),
            defer_loading: false,
        }
    );
}
