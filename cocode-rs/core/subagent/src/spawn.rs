use cocode_protocol::execution::ExecutionIdentity;
use serde::Deserialize;
use serde::Serialize;

/// Input parameters for spawning a new subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnInput {
    /// The agent type to spawn (must match a registered `AgentDefinition`).
    pub agent_type: String,

    /// The prompt or task description for the subagent.
    pub prompt: String,

    /// Model selection identity for this spawn.
    ///
    /// Determines how the model is resolved:
    /// - `Role(ModelRole)`: Use the model configured for that role
    /// - `Spec(ModelSpec)`: Use a specific provider/model
    /// - `Inherit`: Use the parent agent's model
    /// - `None`: Fall back to definition's identity or parent model
    #[serde(default)]
    pub identity: Option<ExecutionIdentity>,

    /// Override the maximum number of turns.
    #[serde(default)]
    pub max_turns: Option<i32>,

    /// Whether this agent should run in the background.
    #[serde(default)]
    pub run_in_background: bool,

    /// Override the allowed tools for this spawn.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::model::ModelRole;
    use cocode_protocol::model::ModelSpec;

    #[test]
    fn test_spawn_input_defaults() {
        let json = r#"{"agent_type":"bash","prompt":"list files"}"#;
        let input: SpawnInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.agent_type, "bash");
        assert_eq!(input.prompt, "list files");
        assert!(input.identity.is_none());
        assert!(input.max_turns.is_none());
        assert!(!input.run_in_background);
        assert!(input.allowed_tools.is_none());
    }

    #[test]
    fn test_spawn_input_with_identity() {
        let input = SpawnInput {
            agent_type: "explore".to_string(),
            prompt: "find all tests".to_string(),
            identity: Some(ExecutionIdentity::Role(ModelRole::Explore)),
            max_turns: Some(20),
            run_in_background: true,
            allowed_tools: Some(vec!["Read".to_string(), "Glob".to_string()]),
        };
        let json = serde_json::to_string(&input).expect("serialize");
        let back: SpawnInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.agent_type, "explore");
        assert!(back.run_in_background);
        assert!(matches!(
            back.identity,
            Some(ExecutionIdentity::Role(ModelRole::Explore))
        ));
    }

    #[test]
    fn test_spawn_input_inherit_identity() {
        let input = SpawnInput {
            agent_type: "bash".to_string(),
            prompt: "test".to_string(),
            identity: Some(ExecutionIdentity::Inherit),
            max_turns: None,
            run_in_background: false,
            allowed_tools: None,
        };
        assert!(matches!(input.identity, Some(ExecutionIdentity::Inherit)));
    }

    #[test]
    fn test_spawn_input_spec_identity() {
        let spec = ModelSpec::new("anthropic", "claude-opus-4");
        let input = SpawnInput {
            agent_type: "general".to_string(),
            prompt: "test".to_string(),
            identity: Some(ExecutionIdentity::Spec(spec.clone())),
            max_turns: None,
            run_in_background: false,
            allowed_tools: None,
        };
        if let Some(ExecutionIdentity::Spec(s)) = &input.identity {
            assert_eq!(s, &spec);
        } else {
            panic!("Expected Spec identity");
        }
    }
}
