//! Codex-specific MCP tools for sub-agent delegation
//!
//! These tools allow sub-agents to call Codex capabilities via MCP protocol.

use serde_json::Value;

/// Codex MCP tool definitions for sub-agents
#[derive(Debug, Clone)]
pub struct CodexMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

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
                    }
                },
                "required": ["path"]
            }),
        }
    }

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
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    /// Codebase search tool (safe, read-only)
    pub fn codebase_search() -> Self {
        Self {
            name: "codex_codebase_search".to_string(),
            description: "Semantic code search using Codex".to_string(),
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
                        "description": "Directories to search in"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    /// Apply patch tool (requires write permission)
    pub fn apply_patch() -> Self {
        Self {
            name: "codex_apply_patch".to_string(),
            description: "Apply a code patch using Codex".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "Unified diff patch to apply"
                    }
                },
                "required": ["patch"]
            }),
        }
    }

    /// Shell command tool (requires shell permission)
    pub fn shell() -> Self {
        Self {
            name: "codex_shell".to_string(),
            description: "Execute a shell command via Codex (restricted)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    /// Get all safe (read-only) tools
    pub fn safe_tools() -> Vec<Self> {
        vec![Self::read_file(), Self::grep(), Self::codebase_search()]
    }

    /// Get all tools (including write/shell)
    pub fn all_tools() -> Vec<Self> {
        vec![
            Self::read_file(),
            Self::grep(),
            Self::codebase_search(),
            Self::apply_patch(),
            Self::shell(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_tools_defined() {
        let safe_tools = CodexMcpTool::safe_tools();
        assert_eq!(safe_tools.len(), 3);
        assert_eq!(safe_tools[0].name, "codex_read_file");

        let all_tools = CodexMcpTool::all_tools();
        assert_eq!(all_tools.len(), 5);
    }
}
