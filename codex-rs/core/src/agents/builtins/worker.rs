use crate::agents::AgentDefinition;

const PROMPT: &str = include_str!("../../../gpt-5.1-codex-max_prompt.md");

pub(super) fn definition() -> AgentDefinition {
    AgentDefinition {
        name: "worker".to_string(),
        instructions: Some(PROMPT.to_string()),
        ..Default::default()
    }
}
