use crate::definition::AgentDefinition;
use cocode_protocol::execution::ExecutionIdentity;
use cocode_protocol::model::ModelRole;

/// Plan agent - creates plans without executing modifications.
/// Uses the Plan model role if configured, otherwise inherits from parent.
pub fn plan_agent() -> AgentDefinition {
    AgentDefinition {
        name: "plan".to_string(),
        description: "Planning agent that reasons about tasks without making changes".to_string(),
        agent_type: "plan".to_string(),
        tools: vec![],
        disallowed_tools: vec!["Task".to_string(), "Edit".to_string(), "Write".to_string()],
        identity: Some(ExecutionIdentity::Role(ModelRole::Plan)),
        max_turns: None,
        permission_mode: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_agent() {
        let agent = plan_agent();
        assert_eq!(agent.name, "plan");
        assert_eq!(agent.agent_type, "plan");
        assert!(
            agent.tools.is_empty(),
            "plan agent can use all non-denied tools"
        );
        assert_eq!(agent.disallowed_tools, vec!["Task", "Edit", "Write"]);
        assert!(agent.max_turns.is_none());
        assert!(matches!(
            agent.identity,
            Some(ExecutionIdentity::Role(ModelRole::Plan))
        ));
    }
}
