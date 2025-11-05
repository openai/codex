// SubAgent MCP Tool Definition
use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentToolParam {
    /// Action to perform
    pub action: String,

    /// Agent type (CodeExpert, SecurityExpert, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,

    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// Task ID (for status check)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

pub fn create_subagent_tool() -> Tool {
    Tool {
        name: "codex-subagent".to_string(),
        title: Some("SubAgent Tool".to_string()),
        description: Some("Manage and interact with Codex subagents. Available actions: start_task, check_inbox, get_status, auto_dispatch, get_thinking, get_token_report".to_string()),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform: start_task, check_inbox, get_status, auto_dispatch, get_thinking, get_token_report",
                    "enum": ["start_task", "check_inbox", "get_status", "auto_dispatch", "get_thinking", "get_token_report"]
                },
                "agent_type": {
                    "type": "string",
                    "description": "Agent type: CodeExpert, SecurityExpert, TestingExpert, DocsExpert, DeepResearcher, DebugExpert, PerformanceExpert, General",
                    "enum": ["CodeExpert", "SecurityExpert", "TestingExpert", "DocsExpert", "DeepResearcher", "DebugExpert", "PerformanceExpert", "General"]
                },
                "task": {
                    "type": "string",
                    "description": "Task description for the subagent"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task ID for status check or thinking process retrieval"
                }
            })),
            required: Some(vec!["action".to_string()]),
        },
        output_schema: None,
        annotations: None,
    }
}
