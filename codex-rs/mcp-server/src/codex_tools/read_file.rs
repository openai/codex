//! Read file tool definition

use super::CodexMcpTool;

impl CodexMcpTool {
    /// Read file tool (safe, read-only)
    pub fn read_file() -> Self {
        Self {
            name: "codex_read_file".to_string(),
            description: "Read a file from the workspace using Codex".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (optional)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of lines to read (optional)"
                    }
                },
                "required": ["path"]
            }),
        }
    }
}
