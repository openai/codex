use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_DEFAULT: &str =
    include_str!("../../templates/collaboration_mode/default.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    vec![plan_preset(), default_preset()]
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

fn default_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Default".to_string(),
        mode: Some(ModeKind::Default),
        model: None,
        reasoning_effort: None,
        developer_instructions: Some(Some(COLLABORATION_MODE_DEFAULT.to_string())),
    }
}
