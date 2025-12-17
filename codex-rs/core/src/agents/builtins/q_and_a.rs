use codex_protocol::openai_models::ReasoningEffort;
use crate::agents::AgentDefinition;

const PROMPT: &str = include_str!("../../../templates/agents/q_and_a.md");

pub(super) fn definition() -> AgentDefinition {
    AgentDefinition {
        name: "q_and_a".to_string(),
        instructions: Some(PROMPT.to_string()),
        read_only: true,
        model: Some("gpt-5.2".to_string()),
        reasoning_effort: Some(ReasoningEffort::High),
        ..Default::default()
    }
}
