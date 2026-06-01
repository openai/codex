use super::LoadableToolSpec;
use super::ResponsesApiNamespace;
use super::ResponsesApiNamespaceTool;
use super::ResponsesApiTool;
use super::dynamic_tool_to_responses_api_tool;
use super::mcp_tool_to_deferred_responses_api_tool;
use super::tool_definition_to_responses_api_tool;
use crate::JsonSchema;
use crate::ToolDefinition;
use crate::ToolName;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

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
    let tool = DynamicToolSpec {
        namespace: None,
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

#[test]
fn strict_responses_api_tool_serializes_valid_schema() {
    let tool = strict_tool(JsonSchema::object(
        BTreeMap::from([(
            "order_id".to_string(),
            JsonSchema::string(/*description*/ None),
        )]),
        Some(vec!["order_id".to_string()]),
        Some(false.into()),
    ));

    tool.validate_for_responses_api()
        .expect("strict schema should validate");

    assert_eq!(
        serde_json::to_value(tool).expect("serialize strict tool"),
        json!({
            "name": "lookup_order",
            "description": "Look up an order",
            "strict": true,
            "parameters": {
                "type": "object",
                "properties": {
                    "order_id": { "type": "string" }
                },
                "required": ["order_id"],
                "additionalProperties": false,
            },
        })
    );
}

#[test]
fn strict_responses_api_tool_rejects_invalid_schemas() {
    let cases = [
        (
            "non-object root",
            JsonSchema::any_of(
                vec![JsonSchema::string(/*description*/ None)],
                /*description*/ None,
            ),
            "strict tool parameters must be an object schema",
        ),
        (
            "missing required",
            JsonSchema::object(BTreeMap::new(), /*required*/ None, Some(false.into())),
            "strict object schemas must include `required`",
        ),
        (
            "missing additionalProperties",
            JsonSchema::object(
                BTreeMap::new(),
                Some(Vec::new()),
                /*additional_properties*/ None,
            ),
            "strict object schemas must set `additionalProperties` to false",
        ),
        (
            "true additionalProperties",
            JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(true.into())),
            "strict object schemas must set `additionalProperties` to false",
        ),
        (
            "schema additionalProperties",
            JsonSchema::object(
                BTreeMap::new(),
                Some(Vec::new()),
                Some(JsonSchema::string(/*description*/ None).into()),
            ),
            "strict object schemas must set `additionalProperties` to false",
        ),
        (
            "property omitted from required",
            JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(Vec::new()),
                Some(false.into()),
            ),
            "strict object schemas must list every property in `required`; missing `order_id`",
        ),
        (
            "nested object missing required",
            JsonSchema::object(
                BTreeMap::from([(
                    "customer".to_string(),
                    JsonSchema::object(
                        BTreeMap::from([(
                            "name".to_string(),
                            JsonSchema::string(/*description*/ None),
                        )]),
                        /*required*/ None,
                        Some(false.into()),
                    ),
                )]),
                Some(vec!["customer".to_string()]),
                Some(false.into()),
            ),
            "strict object schemas must include `required`",
        ),
    ];

    for (name, parameters, expected) in cases {
        let err = match strict_tool(parameters).validate_for_responses_api() {
            Ok(()) => panic!("{name} should fail validation"),
            Err(err) => err,
        };

        assert_eq!(err.to_string(), expected, "{name}");
    }
}

fn strict_tool(parameters: JsonSchema) -> ResponsesApiTool {
    ResponsesApiTool {
        name: "lookup_order".to_string(),
        description: "Look up an order".to_string(),
        strict: true,
        defer_loading: None,
        parameters,
        output_schema: None,
    }
}
