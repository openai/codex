use crate::definition::AgentDefinition;
use cocode_protocol::execution::ExecutionIdentity;

/// General-purpose agent with access to all tools.
/// Inherits model from parent.
pub fn general_agent() -> AgentDefinition {
    AgentDefinition {
        name: "general".to_string(),
        description: "General-purpose coding agent with access to all tools".to_string(),
        agent_type: "general".to_string(),
        tools: vec![],
        disallowed_tools: vec![],
        identity: Some(ExecutionIdentity::Inherit),
        max_turns: None,
        permission_mode: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_agent() {
        let agent = general_agent();
        assert_eq!(agent.name, "general");
        assert_eq!(agent.agent_type, "general");
        assert!(agent.tools.is_empty(), "general agent has all tools");
        assert!(agent.disallowed_tools.is_empty());
        assert!(agent.max_turns.is_none());
        assert!(matches!(agent.identity, Some(ExecutionIdentity::Inherit)));
    }
}
