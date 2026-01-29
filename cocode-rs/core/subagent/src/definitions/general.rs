use crate::definition::AgentDefinition;

/// General-purpose agent with access to all tools.
pub fn general_agent() -> AgentDefinition {
    AgentDefinition {
        name: "general".to_string(),
        description: "General-purpose coding agent with access to all tools".to_string(),
        agent_type: "general".to_string(),
        tools: vec![],
        disallowed_tools: vec![],
        model: None,
        max_turns: None,
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
        assert!(agent.model.is_none());
    }
}
