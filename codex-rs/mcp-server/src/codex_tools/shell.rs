//! Shell command tool definition

use super::CodexMcpTool;

impl CodexMcpTool {
    /// Shell command tool (requires shell permission)
    pub fn shell() -> Self {
        Self {
            name: "codex_shell".to_string(),
            description: "Execute a shell command via Codex (restricted, requires approval)"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory for command execution (optional)"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Command timeout in seconds (optional)"
                    }
                },
                "required": ["command"]
            }),
        }
    }
}
