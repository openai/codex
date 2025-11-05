//! Grep tool definition

use super::CodexMcpTool;

impl CodexMcpTool {
    /// Grep tool (safe, read-only)
    pub fn grep() -> Self {
        Self {
            name: "codex_grep".to_string(),
            description: "Search for patterns in files using Codex grep".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to search in (file or directory)"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "description": "Case insensitive search (optional)"
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["content", "files_with_matches", "count"],
                        "description": "Output mode (optional)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }
}
