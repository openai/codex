use super::*;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use serde_json::json;
use std::sync::Arc;

#[test]
fn serialize_tool_search_output_tools_groups_results_by_namespace() {
    let entries = [
        (
            "mcp__codex_apps__calendar_create_event".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "_create_event".to_string(),
                tool_namespace: "mcp__codex_apps__calendar".to_string(),
                tool: Tool {
                    name: "calendar-create-event".to_string().into(),
                    title: None,
                    description: Some("Create a calendar event.".into()),
                    input_schema: Arc::new(JsonObject::from_iter([(
                        "type".to_string(),
                        json!("object"),
                    )])),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: Some("calendar".to_string()),
                connector_name: Some("Calendar".to_string()),
                plugin_display_names: Vec::new(),
                connector_description: Some("Plan events".to_string()),
            },
        ),
        (
            "mcp__codex_apps__gmail_read_email".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "_read_email".to_string(),
                tool_namespace: "mcp__codex_apps__gmail".to_string(),
                tool: Tool {
                    name: "gmail-read-email".to_string().into(),
                    title: None,
                    description: Some("Read an email.".into()),
                    input_schema: Arc::new(JsonObject::from_iter([(
                        "type".to_string(),
                        json!("object"),
                    )])),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: Some("gmail".to_string()),
                connector_name: Some("Gmail".to_string()),
                plugin_display_names: Vec::new(),
                connector_description: Some("Read mail".to_string()),
            },
        ),
        (
            "mcp__codex_apps__calendar_list_events".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "_list_events".to_string(),
                tool_namespace: "mcp__codex_apps__calendar".to_string(),
                tool: Tool {
                    name: "calendar-list-events".to_string().into(),
                    title: None,
                    description: Some("List calendar events.".into()),
                    input_schema: Arc::new(JsonObject::from_iter([(
                        "type".to_string(),
                        json!("object"),
                    )])),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: Some("calendar".to_string()),
                connector_name: Some("Calendar".to_string()),
                plugin_display_names: Vec::new(),
                connector_description: Some("Plan events".to_string()),
            },
        ),
    ];

    let tools = serialize_tool_search_output_tools(&[&entries[0], &entries[1], &entries[2]])
        .expect("serialize tool search output");

    assert_eq!(
        tools,
        vec![
            ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                name: "mcp__codex_apps__calendar".to_string(),
                description: "Plan events".to_string(),
                tools: vec![
                    ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                        name: "_create_event".to_string(),
                        description: "Create a calendar event.".to_string(),
                        strict: false,
                        defer_loading: Some(true),
                        parameters: crate::tools::spec::JsonSchema::Object {
                            properties: Default::default(),
                            required: None,
                            additional_properties: None,
                        },
                        output_schema: None,
                    }),
                    ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                        name: "_list_events".to_string(),
                        description: "List calendar events.".to_string(),
                        strict: false,
                        defer_loading: Some(true),
                        parameters: crate::tools::spec::JsonSchema::Object {
                            properties: Default::default(),
                            required: None,
                            additional_properties: None,
                        },
                        output_schema: None,
                    }),
                ],
            }),
            ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                name: "mcp__codex_apps__gmail".to_string(),
                description: "Read mail".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "_read_email".to_string(),
                    description: "Read an email.".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: crate::tools::spec::JsonSchema::Object {
                        properties: Default::default(),
                        required: None,
                        additional_properties: None,
                    },
                    output_schema: None,
                })],
            })
        ]
    );
}

#[test]
fn serialize_tool_search_output_tools_falls_back_to_connector_name_description() {
    let entries = [(
        "mcp__codex_apps__gmail_batch_read_email".to_string(),
        ToolInfo {
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            tool_name: "_batch_read_email".to_string(),
            tool_namespace: "mcp__codex_apps__gmail".to_string(),
            tool: Tool {
                name: "gmail-batch-read-email".to_string().into(),
                title: None,
                description: Some("Read multiple emails.".into()),
                input_schema: Arc::new(JsonObject::from_iter([(
                    "type".to_string(),
                    json!("object"),
                )])),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: Some("connector_gmail_456".to_string()),
            connector_name: Some("Gmail".to_string()),
            plugin_display_names: Vec::new(),
            connector_description: None,
        },
    )];

    let tools = serialize_tool_search_output_tools(&[&entries[0]]).expect("serialize");

    assert_eq!(
        tools,
        vec![ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
            name: "mcp__codex_apps__gmail".to_string(),
            description: "Tools for working with Gmail.".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "_batch_read_email".to_string(),
                description: "Read multiple emails.".to_string(),
                strict: false,
                defer_loading: Some(true),
                parameters: crate::tools::spec::JsonSchema::Object {
                    properties: Default::default(),
                    required: None,
                    additional_properties: None,
                },
                output_schema: None,
            })],
        })]
    );
}
