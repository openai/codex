use super::mcp_call_tool_result_output_schema;
use super::mcp_tool_definition;
use crate::ToolDefinition;
use crate::ToolName;
use codex_tool_api::FunctionToolSpec;
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
        mcp_tool_definition(ToolName::plain("no_props"), &tool),
        ToolDefinition::new(
            ToolName::plain("no_props"),
            FunctionToolSpec {
                name: "no_props".to_string(),
                description: "No properties".to_string(),
                strict: false,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            (),
        )
        .with_output_schema(mcp_call_tool_result_output_schema(serde_json::json!({})))
    );
}

#[test]
fn mcp_tool_definition_uses_callable_name() {
    let tool = mcp_tool(
        "calendar_create_event",
        "Create an event",
        serde_json::json!({
            "type": "object"
        }),
    );

    let definition = mcp_tool_definition(
        ToolName::namespaced("mcp__codex_apps__calendar", "_create_event"),
        &tool,
    );

    assert_eq!(definition.spec().name, "_create_event");
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
        mcp_tool_definition(ToolName::plain("with_output"), &tool),
        ToolDefinition::new(
            ToolName::plain("with_output"),
            FunctionToolSpec {
                name: "with_output".to_string(),
                description: "Has output schema".to_string(),
                strict: false,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            (),
        )
        .with_output_schema(mcp_call_tool_result_output_schema(serde_json::json!({
            "properties": {
                "result": {
                    "properties": {
                        "nested": {}
                    }
                }
            },
            "required": ["result"]
        })))
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
        mcp_tool_definition(ToolName::plain("with_enum_output"), &tool),
        ToolDefinition::new(
            ToolName::plain("with_enum_output"),
            FunctionToolSpec {
                name: "with_enum_output".to_string(),
                description: "Has enum output schema".to_string(),
                strict: false,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            (),
        )
        .with_output_schema(mcp_call_tool_result_output_schema(serde_json::json!({
            "enum": ["ok", "error"]
        })))
    );
}
