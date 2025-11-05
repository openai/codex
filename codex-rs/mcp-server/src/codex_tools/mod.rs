//! Codex-specific MCP tools for sub-agent delegation
//!
//! These tools allow sub-agents to call Codex capabilities via MCP protocol.

use serde_json::Value;

// Tool implementations
mod apply_patch;
mod codebase_search;
mod grep;
mod read_file;
mod shell;

/// Codex MCP tool definitions for sub-agents
#[derive(Debug, Clone)]
pub struct CodexMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl CodexMcpTool {
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
