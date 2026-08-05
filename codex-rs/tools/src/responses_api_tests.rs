use super::FreeformTool;
use super::LoadableToolSpec;
use super::ResponsesApiNamespace;
use super::ResponsesApiNamespaceTool;
use super::ResponsesApiTool;
use super::agent_plugin_mcp_tool_to_responses_api_tool;
use super::dynamic_tool_to_responses_api_tool;
use super::mcp_tool_to_deferred_responses_api_tool;
use super::tool_definition_to_responses_api_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolName;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn freeform_tool_deferral_matches_function_tool_wire_shape() {
    let mut expected_wire_shape = json!({
        "name": "apply_patch",
        "description": "Apply a patch",
        "format": {
            "type": "grammar",
            "syntax": "lark",
            "definition": "start: \"patch\"",
        },
    });

    let mut tool: FreeformTool = serde_json::from_value(expected_wire_shape.clone())
        .expect("deserialize legacy freeform tool");

    assert_eq!(tool.defer_loading, None);
    assert_eq!(
        serde_json::to_value(&tool).expect("serialize eager freeform tool"),
        expected_wire_shape
    );

    tool.defer_loading = Some(true);
    expected_wire_shape["defer_loading"] = json!(true);
    assert_eq!(
        serde_json::to_value(tool).expect("serialize deferred freeform tool"),
        expected_wire_shape
    );
}

#[test]
fn tool_definition_to_responses_api_tool_omits_false_defer_loading() {
    assert_eq!(
        tool_definition_to_responses_api_tool(ToolDefinition {
            name: "lookup_order".to_string(),
            description: "Look up an order".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(false.into())
            ),
            output_schema: Some(json!({"type": "object"})),
            defer_loading: false,
        }),
        ResponsesApiTool {
            name: "lookup_order".to_string(),
            description: "Look up an order".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(false.into())
            ),
            output_schema: Some(json!({"type": "object"})),
        }
    );
}

#[test]
fn dynamic_tool_to_responses_api_tool_preserves_defer_loading() {
    let tool = DynamicToolFunctionSpec {
        name: "lookup_order".to_string(),
        description: "Look up an order".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "order_id": {"type": "string"}
            },
            "required": ["order_id"],
            "additionalProperties": false,
        }),
        defer_loading: true,
    };

    assert_eq!(
        dynamic_tool_to_responses_api_tool(&tool).expect("convert dynamic tool"),
        ResponsesApiTool {
            name: "lookup_order".to_string(),
            description: "Look up an order".to_string(),
            strict: false,
            defer_loading: Some(true),
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(false.into())
            ),
            output_schema: None,
        }
    );
}

#[test]
fn mcp_tool_to_deferred_responses_api_tool_sets_defer_loading() {
    let tool = rmcp::model::Tool::new(
        "lookup_order",
        "Look up an order",
        std::sync::Arc::new(rmcp::model::object(json!({
            "type": "object",
            "properties": {
                "order_id": {"type": "string"}
            },
            "required": ["order_id"],
            "additionalProperties": false,
        }))),
    );

    assert_eq!(
        mcp_tool_to_deferred_responses_api_tool(
            &ToolName::namespaced("mcp__codex_apps__", "lookup_order"),
            &tool,
        )
        .expect("convert deferred tool"),
        ResponsesApiTool {
            name: "lookup_order".to_string(),
            description: "Look up an order".to_string(),
            strict: false,
            defer_loading: Some(true),
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(false.into())
            ),
            output_schema: None,
        }
    );
}

#[test]
fn agent_plugin_mcp_tool_uses_fallback_for_oversized_schema() {
    let properties: serde_json::Map<String, serde_json::Value> = (0..1_024)
        .map(|index| (format!("property_{index}"), json!({"type": "string"})))
        .collect();
    let tool = rmcp::model::Tool::new(
        "oversized",
        "Large schema",
        std::sync::Arc::new(rmcp::model::object(json!({
            "type": "object",
            "properties": properties,
        }))),
    );

    assert_eq!(
        agent_plugin_mcp_tool_to_responses_api_tool(&ToolName::from("oversized"), &tool)
            .expect("Agent Plugin MCP tool should use a fallback schema")
            .parameters,
        JsonSchema::object(BTreeMap::new(), /*required*/ None, Some(true.into()))
    );
}

#[test]
fn legacy_mcp_tool_accepts_oversized_schema() {
    let properties: serde_json::Map<String, serde_json::Value> = (0..1_024)
        .map(|index| (format!("property_{index}"), json!({"type": "string"})))
        .collect();
    let tool = rmcp::model::Tool::new(
        "oversized",
        "Large legacy schema",
        std::sync::Arc::new(rmcp::model::object(json!({
            "type": "object",
            "properties": properties,
        }))),
    );

    super::mcp_tool_to_responses_api_tool(&ToolName::from("oversized"), &tool)
        .expect("legacy MCP conversion must preserve existing acceptance");
}

#[test]
fn loadable_tool_spec_namespace_serializes_with_deferred_child_tools() {
    let namespace = LoadableToolSpec::Namespace(ResponsesApiNamespace {
        name: "mcp__codex_apps__calendar".to_string(),
        description: "Plan events".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "create_event".to_string(),
            description: "Create a calendar event.".to_string(),
            strict: false,
            defer_loading: Some(true),
            parameters: JsonSchema::object(
                Default::default(),
                /*required*/ None,
                /*additional_properties*/ None,
            ),
            output_schema: None,
        })],
    });

    let value = serde_json::to_value(namespace).expect("serialize namespace");

    assert_eq!(
        value,
        json!({
            "type": "namespace",
            "name": "mcp__codex_apps__calendar",
            "description": "Plan events",
            "tools": [
                {
                    "type": "function",
                    "name": "create_event",
                    "description": "Create a calendar event.",
                    "strict": false,
                    "defer_loading": true,
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        })
    );
}
