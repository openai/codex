use super::mcp_call_tool_result_output_schema;
use super::mcp_tool_to_tool_definition;
use super::parse_mcp_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
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
fn mcp_tool_to_tool_definition_uses_canonical_tool_name() {
    let tool = mcp_tool(
        "raw_lookup",
        "Look up an order",
        serde_json::json!({
            "type": "object"
        }),
    );

    assert_eq!(
        mcp_tool_to_tool_definition(
            &ToolName::namespaced("mcp__orders__", "lookup_order"),
            &tool,
        )
        .expect("convert MCP tool"),
        ToolDefinition {
            name: ToolName::namespaced("mcp__orders__", "lookup_order"),
            description: "Look up an order".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Mcp,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
        }
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
            name: ToolName::plain("no_props"),
            description: "No properties".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Mcp,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
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
        ToolDefinition {
            name: ToolName::plain("with_output"),
            description: "Has output schema".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
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
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Mcp,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
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
            name: ToolName::plain("with_enum_output"),
            description: "Has enum output schema".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({
                "enum": ["ok", "error"]
            }))),
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Mcp,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
        }
    );
}
