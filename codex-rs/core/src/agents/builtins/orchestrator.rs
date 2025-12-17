use crate::agents::AgentDefinition;

const PROMPT: &str = include_str!("../../../templates/agents/orchestrator.md");

pub(super) fn definition() -> AgentDefinition {
    AgentDefinition {
        name: "orchestrator".to_string(),
        instructions: Some(PROMPT.to_string()),
        sub_agents: ["worker", "reviewer", "q_and_a"].iter().map(|s| s.to_string()).collect(),
        read_only: true,
        ..Default::default()
    }
}
