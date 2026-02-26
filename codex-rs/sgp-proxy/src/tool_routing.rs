use std::collections::HashSet;

/// Where a tool call should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRoute {
    /// The Agentex agent handles this tool internally.
    Agent,
    /// The tool call is emitted as a FunctionCall for Codex to execute locally.
    CodexLocal,
}

/// Determine whether a tool call should be handled by the agent or by Codex.
pub fn route_tool(name: &str, agent_tools: &HashSet<String>) -> ToolRoute {
    if agent_tools.contains(name) {
        ToolRoute::Agent
    } else {
        ToolRoute::CodexLocal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_agent_tool() {
        let mut tools = HashSet::new();
        tools.insert("search".to_string());
        assert_eq!(route_tool("search", &tools), ToolRoute::Agent);
    }

    #[test]
    fn test_route_codex_local_tool() {
        let tools = HashSet::new();
        assert_eq!(route_tool("read_file", &tools), ToolRoute::CodexLocal);
    }

    #[test]
    fn test_route_unknown_tool_is_codex_local() {
        let mut tools = HashSet::new();
        tools.insert("search".to_string());
        assert_eq!(route_tool("write_file", &tools), ToolRoute::CodexLocal);
    }
}
