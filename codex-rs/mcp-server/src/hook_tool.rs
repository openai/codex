// Hook MCP Tool Definition
use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookToolParam {
    /// Hook event type
    pub event: String,

    /// Optional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

pub fn create_hook_tool() -> Tool {
    Tool {
        name: "codex-hook".to_string(),
        title: Some("Hook Tool".to_string()),
        description: Some("Execute hooks for lifecycle events. Available events: on_task_start, on_task_complete, on_error, on_task_abort, on_subagent_start, on_subagent_complete, on_session_start, on_session_end, on_patch_apply, on_command_exec".to_string()),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "event": {
                    "type": "string",
                    "description": "Hook event to trigger",
                    "enum": [
                        "on_task_start",
                        "on_task_complete",
                        "on_error",
                        "on_task_abort",
                        "on_subagent_start",
                        "on_subagent_complete",
                        "on_session_start",
                        "on_session_end",
                        "on_patch_apply",
                        "on_command_exec"
                    ]
                },
                "context": {
                    "type": "string",
                    "description": "Optional context information for the hook"
                }
            })),
            required: Some(vec!["event".to_string()]),
        },
        output_schema: None,
        annotations: None,
    }
}
