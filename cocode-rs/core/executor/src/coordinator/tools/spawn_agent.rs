use serde::Deserialize;
use serde::Serialize;

/// Input for the spawn_agent tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentInput {
    /// Model to use for the spawned agent.
    pub model: String,

    /// Initial prompt or task description.
    pub prompt: String,

    /// List of tools to make available.
    #[serde(default)]
    pub tools: Vec<String>,

    /// Maximum number of turns the agent may execute.
    #[serde(default)]
    pub max_turns: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_agent_input_defaults() {
        let json = r#"{"model":"claude-3","prompt":"hello"}"#;
        let input: SpawnAgentInput = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.model, "claude-3");
        assert_eq!(input.prompt, "hello");
        assert!(input.tools.is_empty());
        assert!(input.max_turns.is_none());
    }

    #[test]
    fn test_spawn_agent_input_full() {
        let input = SpawnAgentInput {
            model: "claude-3".to_string(),
            prompt: "build feature".to_string(),
            tools: vec!["Bash".to_string(), "Edit".to_string()],
            max_turns: Some(20),
        };
        let json = serde_json::to_string(&input).expect("serialize");
        let back: SpawnAgentInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.model, "claude-3");
        assert_eq!(back.tools.len(), 2);
        assert_eq!(back.max_turns, Some(20));
    }
}
