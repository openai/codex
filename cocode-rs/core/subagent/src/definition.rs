use cocode_protocol::execution::ExecutionIdentity;
use serde::Deserialize;
use serde::Serialize;

/// Declarative definition of a subagent type.
///
/// Each definition specifies the agent's name, description, allowed/disallowed
/// tools, and optional model and turn limit overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Unique name for this agent type (e.g. "bash", "explore").
    pub name: String,

    /// Human-readable description of the agent's purpose.
    pub description: String,

    /// Agent type identifier used for spawning.
    pub agent_type: String,

    /// Allowed tools (empty means all tools are available).
    #[serde(default)]
    pub tools: Vec<String>,

    /// Tools explicitly denied to this agent.
    #[serde(default)]
    pub disallowed_tools: Vec<String>,

    /// Model selection identity for this agent type.
    ///
    /// Determines how the model is resolved:
    /// - `Role(ModelRole)`: Use the model configured for that role
    /// - `Spec(ModelSpec)`: Use a specific provider/model
    /// - `Inherit`: Use the parent agent's model
    /// - `None`: Fall back to parent model (same as Inherit)
    #[serde(default)]
    pub identity: Option<ExecutionIdentity>,

    /// Override the maximum number of turns for this agent.
    #[serde(default)]
    pub max_turns: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::model::ModelRole;

    #[test]
    fn test_agent_definition_defaults() {
        let json = r#"{"name":"test","description":"A test agent","agent_type":"test"}"#;
        let def: AgentDefinition = serde_json::from_str(json).expect("deserialize");
        assert_eq!(def.name, "test");
        assert!(def.tools.is_empty());
        assert!(def.disallowed_tools.is_empty());
        assert!(def.identity.is_none());
        assert!(def.max_turns.is_none());
    }

    #[test]
    fn test_agent_definition_full() {
        let def = AgentDefinition {
            name: "bash".to_string(),
            description: "Bash executor".to_string(),
            agent_type: "bash".to_string(),
            tools: vec!["Bash".to_string()],
            disallowed_tools: vec!["Edit".to_string()],
            identity: Some(ExecutionIdentity::Role(ModelRole::Main)),
            max_turns: Some(10),
        };
        let json = serde_json::to_string(&def).expect("serialize");
        let back: AgentDefinition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.name, "bash");
        assert_eq!(back.tools, vec!["Bash"]);
        assert_eq!(back.disallowed_tools, vec!["Edit"]);
        assert!(matches!(
            back.identity,
            Some(ExecutionIdentity::Role(ModelRole::Main))
        ));
        assert_eq!(back.max_turns, Some(10));
    }

    #[test]
    fn test_agent_definition_with_identity() {
        let def = AgentDefinition {
            name: "explore".to_string(),
            description: "Explorer".to_string(),
            agent_type: "explore".to_string(),
            tools: vec![],
            disallowed_tools: vec![],
            identity: Some(ExecutionIdentity::Role(ModelRole::Explore)),
            max_turns: None,
        };
        assert!(matches!(
            def.identity,
            Some(ExecutionIdentity::Role(ModelRole::Explore))
        ));
    }
}
