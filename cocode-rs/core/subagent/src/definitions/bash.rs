use crate::definition::AgentDefinition;
use cocode_protocol::execution::ExecutionIdentity;

/// Bash agent - executes shell commands.
pub fn bash_agent() -> AgentDefinition {
    AgentDefinition {
        name: "bash".to_string(),
        description: "Executes shell commands via Bash".to_string(),
        agent_type: "bash".to_string(),
        tools: vec!["Bash".to_string()],
        disallowed_tools: vec![],
        identity: Some(ExecutionIdentity::Inherit),
        max_turns: Some(10),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_agent() {
        let agent = bash_agent();
        assert_eq!(agent.name, "bash");
        assert_eq!(agent.agent_type, "bash");
        assert_eq!(agent.tools, vec!["Bash"]);
        assert!(agent.disallowed_tools.is_empty());
        assert_eq!(agent.max_turns, Some(10));
        assert!(matches!(agent.identity, Some(ExecutionIdentity::Inherit)));
    }
}
