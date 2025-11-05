//! Codebase semantic search tool definition

use super::CodexMcpTool;

impl CodexMcpTool {
    /// Codebase search tool (safe, read-only)
    pub fn codebase_search() -> Self {
        Self {
            name: "codex_codebase_search".to_string(),
            description: "Semantic code search using Codex AI-powered search".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language query for semantic search"
                    },
                    "target_directories": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Directories to search in (optional, empty for all)"
                    },
                    "explanation": {
                        "type": "string",
                        "description": "Why this search is being performed (optional)"
                    }
                },
                "required": ["query"]
            }),
        }
    }
}
