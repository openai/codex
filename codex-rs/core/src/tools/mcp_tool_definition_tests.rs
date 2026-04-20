use super::*;
use codex_tools::JsonSchema;
use codex_tools::ToolExecution;
use codex_tools::ToolName;
use codex_tools::mcp_call_tool_result_output_schema;
use pretty_assertions::assert_eq;
use rmcp::model::Tool;
use std::collections::BTreeMap;
use std::sync::Arc;

fn tool_info() -> ToolInfo {
    ToolInfo {
        server_name: "codex_apps".to_string(),
        callable_name: "create_event".to_string(),
        callable_namespace: "mcp__calendar__".to_string(),
        server_instructions: Some("Use the calendar carefully.".to_string()),
        tool: Tool {
            name: "calendar-create-event".to_string().into(),
            title: None,
            description: Some("Create events".to_string().into()),
            input_schema: Arc::new(rmcp::model::object(serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"}
                },
                "additionalProperties": false
            }))),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        plugin_display_names: vec!["calendar-plugin".to_string()],
        connector_description: None,
    }
}

#[test]
fn eager_mcp_tool_info_to_tool_definition_uses_server_instructions_for_namespace() {
    assert_eq!(
        mcp_tool_info_to_tool_definition(&tool_info(), ToolLoadingPolicy::Eager)
            .expect("convert MCP tool info"),
        ToolDefinition {
            name: ToolName::namespaced("mcp__calendar__", "create_event"),
            description: "Create events".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::from([(
                    "title".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                /*required*/ None,
                Some(false.into())
            ),
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            loading: ToolLoadingPolicy::Eager,
            execution: ToolExecution::Mcp,
            presentation: Some(ToolPresentation {
                namespace_display_name: None,
                namespace_description: Some("Use the calendar carefully.".to_string()),
            }),
            search: None,
            supports_parallel_tool_calls: false,
        }
    );
}

#[test]
fn deferred_mcp_tool_info_to_tool_definition_populates_search_metadata() {
    assert_eq!(
        mcp_tool_info_to_tool_definition(&tool_info(), ToolLoadingPolicy::Deferred)
            .expect("convert MCP tool info"),
        ToolDefinition {
            name: ToolName::namespaced("mcp__calendar__", "create_event"),
            description: "Create events".to_string(),
            input_schema: JsonSchema::object(
                BTreeMap::from([(
                    "title".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                /*required*/ None,
                Some(false.into())
            ),
            output_schema: None,
            loading: ToolLoadingPolicy::Deferred,
            execution: ToolExecution::Mcp,
            presentation: Some(ToolPresentation {
                namespace_display_name: None,
                namespace_description: Some("Tools for working with Calendar.".to_string()),
            }),
            search: Some(ToolSearchMetadata {
                source_name: "Calendar".to_string(),
                source_description: None,
                extra_terms: vec![
                    "create_event".to_string(),
                    "calendar-create-event".to_string(),
                    "codex_apps".to_string(),
                    "Calendar".to_string(),
                    "calendar-plugin".to_string(),
                ],
                limit_bucket: Some("codex_apps".to_string()),
            }),
            supports_parallel_tool_calls: false,
        }
    );
}
