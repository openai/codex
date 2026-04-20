use super::ResponsesApiNamespace;
use super::ResponsesApiNamespaceTool;
use super::ResponsesApiTool;
use super::ToolSearchOutputTool;
use super::dynamic_tool_to_responses_api_tool;
use super::tool_definition_to_responses_api_tool;
use super::tool_definition_to_tool_search_output_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
use crate::ToolPresentation;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn tool_definition_to_responses_api_tool_omits_false_defer_loading() {
    assert_eq!(
        tool_definition_to_responses_api_tool(&ToolDefinition {
            name: ToolName::plain("lookup_order"),
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
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Dynamic,
            presentation: None,
            search: None,
            supports_parallel_tool_calls: false,
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
    let tool = DynamicToolSpec {
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
fn tool_search_output_namespace_serializes_with_deferred_child_tools() {
    let namespace = ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
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

#[test]
fn tool_definition_to_tool_search_output_tool_wraps_namespaced_tools() {
    assert_eq!(
        tool_definition_to_tool_search_output_tool(&ToolDefinition {
            name: ToolName::namespaced("mcp__calendar__", "create_event"),
            description: "Create a calendar event.".to_string(),
            input_schema: JsonSchema::object(
                Default::default(),
                /*required*/ None,
                /*additional_properties*/ None,
            ),
            output_schema: None,
            loading: ToolLoadingPolicy::Deferred,
            execution: ToolExecution::Mcp,
            presentation: Some(ToolPresentation {
                namespace_display_name: None,
                namespace_description: Some("Calendar tools.".to_string()),
            }),
            search: None,
            supports_parallel_tool_calls: false,
        }),
        ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
            name: "mcp__calendar__".to_string(),
            description: "Calendar tools.".to_string(),
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
        })
    );
}
