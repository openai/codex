use crate::definition::AgentDefinition;
use cocode_protocol::execution::ExecutionIdentity;
use cocode_protocol::model::ModelRole;

/// Explore agent - read-only file exploration.
/// Uses the Explore model role if configured, otherwise inherits from parent.
pub fn explore_agent() -> AgentDefinition {
    AgentDefinition {
        name: "explore".to_string(),
        description: "Read-only file exploration agent for codebase navigation".to_string(),
        agent_type: "explore".to_string(),
        tools: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string()],
        disallowed_tools: vec![],
        identity: Some(ExecutionIdentity::Role(ModelRole::Explore)),
        max_turns: Some(20),
        permission_mode: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explore_agent() {
        let agent = explore_agent();
        assert_eq!(agent.name, "explore");
        assert_eq!(agent.agent_type, "explore");
        assert_eq!(agent.tools, vec!["Read", "Glob", "Grep"]);
        assert!(agent.disallowed_tools.is_empty());
        assert_eq!(agent.max_turns, Some(20));
        assert!(matches!(
            agent.identity,
            Some(ExecutionIdentity::Role(ModelRole::Explore))
        ));
    }
}
