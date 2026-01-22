use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_CODE: &str = include_str!("../../templates/collaboration_mode/code.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    vec![plan_preset(), code_preset()]
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    builtin_collaboration_mode_presets()
}

fn plan_preset() -> CollaborationMode {
    CollaborationMode::Plan(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: Some(COLLABORATION_MODE_PLAN.to_string()),
    })
}

fn code_preset() -> CollaborationMode {
    CollaborationMode::Code(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::XHigh),
        developer_instructions: Some(COLLABORATION_MODE_CODE.to_string()),
    })
}
