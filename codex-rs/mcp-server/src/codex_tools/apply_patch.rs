//! Apply patch tool definition

use super::CodexMcpTool;

impl CodexMcpTool {
    /// Apply patch tool (requires write permission)
    pub fn apply_patch() -> Self {
        Self {
            name: "codex_apply_patch".to_string(),
            description: "Apply a code patch using Codex (requires write permission)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "Unified diff patch to apply"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview changes without applying (optional)"
                    }
                },
                "required": ["patch"]
            }),
        }
    }
}
