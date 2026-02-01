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

    /// Override the model used by this agent.
    #[serde(default)]
    pub model: Option<String>,

    /// Override the maximum number of turns for this agent.
    #[serde(default)]
    pub max_turns: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_definition_defaults() {
        let json = r#"{"name":"test","description":"A test agent","agent_type":"test"}"#;
        let def: AgentDefinition = serde_json::from_str(json).expect("deserialize");
        assert_eq!(def.name, "test");
        assert!(def.tools.is_empty());
        assert!(def.disallowed_tools.is_empty());
        assert!(def.model.is_none());
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
            model: Some("claude-3".to_string()),
            max_turns: Some(10),
        };
        let json = serde_json::to_string(&def).expect("serialize");
        let back: AgentDefinition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.name, "bash");
        assert_eq!(back.tools, vec!["Bash"]);
        assert_eq!(back.disallowed_tools, vec!["Edit"]);
        assert_eq!(back.model, Some("claude-3".to_string()));
        assert_eq!(back.max_turns, Some(10));
    }
}
