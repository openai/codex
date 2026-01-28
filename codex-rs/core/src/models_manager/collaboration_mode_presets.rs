use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_AGENT: &str = include_str!("../../templates/collaboration_mode/agent.md");
const COLLABORATION_MODE_PAIR_PROGRAMMING: &str =
    include_str!("../../templates/collaboration_mode/pair_programming.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    vec![plan_preset(), agent_preset(), pair_programming_preset()]
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    builtin_collaboration_mode_presets()
}

fn plan_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Plan".to_string(),
        mode: Some(ModeKind::Plan),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(COLLABORATION_MODE_PLAN.to_string())),
    }
}

fn agent_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Agent".to_string(),
        mode: Some(ModeKind::Agent),
        model: None,
        reasoning_effort: None,
        developer_instructions: Some(Some(COLLABORATION_MODE_AGENT.to_string())),
    }
}

fn pair_programming_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Pair Programming".to_string(),
        mode: Some(ModeKind::PairProgramming),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(COLLABORATION_MODE_PAIR_PROGRAMMING.to_string())),
    }
}
