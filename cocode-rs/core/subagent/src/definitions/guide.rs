use crate::definition::AgentDefinition;

/// Guide agent - reads and navigates documentation and code.
pub fn guide_agent() -> AgentDefinition {
    AgentDefinition {
        name: "guide".to_string(),
        description: "Guided reading agent for documentation and code navigation".to_string(),
        agent_type: "guide".to_string(),
        tools: vec!["Glob".to_string(), "Grep".to_string(), "Read".to_string()],
        disallowed_tools: vec![],
        model: None,
        max_turns: Some(15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guide_agent() {
        let agent = guide_agent();
        assert_eq!(agent.name, "guide");
        assert_eq!(agent.agent_type, "guide");
        assert_eq!(agent.tools, vec!["Glob", "Grep", "Read"]);
        assert!(agent.disallowed_tools.is_empty());
        assert_eq!(agent.max_turns, Some(15));
        assert!(agent.model.is_none());
    }
}
