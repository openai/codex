// Custom Command MCP Tool Definition
use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommandToolParam {
    /// Action: execute, list, info
    pub action: String,

    /// Command name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_name: Option<String>,

    /// Context for command execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

pub fn create_custom_command_tool() -> Tool {
    Tool {
        name: "codex-custom-command".to_string(),
        title: Some("Custom Command Tool".to_string()),
        description: Some("Execute custom commands that call specific subagents. Available actions: execute, list, info. Default commands: analyze_code, security_review, generate_tests, deep_research, debug_issue, optimize_performance, generate_docs".to_string()),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform: execute (run command), list (show all commands), info (get command details)",
                    "enum": ["execute", "list", "info"]
                },
                "command_name": {
                    "type": "string",
                    "description": "Name of the command to execute or get info about",
                    "enum": ["analyze_code", "security_review", "generate_tests", "deep_research", "debug_issue", "optimize_performance", "generate_docs"]
                },
                "context": {
                    "type": "string",
                    "description": "Context or input for the command (e.g., code to analyze, query to research)"
                }
            })),
            required: Some(vec!["action".to_string()]),
        },
        output_schema: None,
        annotations: None,
    }
}
