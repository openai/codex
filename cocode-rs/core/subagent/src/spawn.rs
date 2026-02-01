use serde::Deserialize;
use serde::Serialize;

/// Input parameters for spawning a new subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnInput {
    /// The agent type to spawn (must match a registered `AgentDefinition`).
    pub agent_type: String,

    /// The prompt or task description for the subagent.
    pub prompt: String,

    /// Override the model for this specific spawn.
    #[serde(default)]
    pub model: Option<String>,

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

    #[test]
    fn test_spawn_input_defaults() {
        let json = r#"{"agent_type":"bash","prompt":"list files"}"#;
        let input: SpawnInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.agent_type, "bash");
        assert_eq!(input.prompt, "list files");
        assert!(input.model.is_none());
        assert!(input.max_turns.is_none());
        assert!(!input.run_in_background);
        assert!(input.allowed_tools.is_none());
    }

    #[test]
    fn test_spawn_input_full() {
        let input = SpawnInput {
            agent_type: "explore".to_string(),
            prompt: "find all tests".to_string(),
            model: Some("claude-3".to_string()),
            max_turns: Some(20),
            run_in_background: true,
            allowed_tools: Some(vec!["Read".to_string(), "Glob".to_string()]),
        };
        let json = serde_json::to_string(&input).expect("serialize");
        let back: SpawnInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.agent_type, "explore");
        assert!(back.run_in_background);
        assert_eq!(
            back.allowed_tools,
            Some(vec!["Read".to_string(), "Glob".to_string()])
        );
    }
}
