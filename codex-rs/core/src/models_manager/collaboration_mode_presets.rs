use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_PAIR_PROGRAMMING: &str =
    include_str!("../../templates/collaboration_mode/pair_programming.md");
const COLLABORATION_MODE_EXECUTE: &str =
    include_str!("../../templates/collaboration_mode/execute.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    vec![plan_preset(), pair_programming_preset(), execute_preset()]
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    builtin_collaboration_mode_presets()
}

fn plan_preset() -> CollaborationMode {
    CollaborationMode::Plan(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::High),
        developer_instructions: Some(COLLABORATION_MODE_PLAN.to_string()),
    })
}

fn pair_programming_preset() -> CollaborationMode {
    CollaborationMode::PairProgramming(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: Some(COLLABORATION_MODE_PAIR_PROGRAMMING.to_string()),
    })
}

fn execute_preset() -> CollaborationMode {
    CollaborationMode::Execute(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::High),
        developer_instructions: Some(COLLABORATION_MODE_EXECUTE.to_string()),
    })
}
