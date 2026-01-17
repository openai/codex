use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationMode> {
    vec![plan_preset(), collaborate_preset(), execute_preset()]
}

fn plan_preset() -> CollaborationMode {
    CollaborationMode::Plan(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: None,
    })
}

fn collaborate_preset() -> CollaborationMode {
    CollaborationMode::Collaborate(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::Medium),
        developer_instructions: None,
    })
}

fn execute_preset() -> CollaborationMode {
    CollaborationMode::Execute(Settings {
        model: "gpt-5.2-codex".to_string(),
        reasoning_effort: Some(ReasoningEffort::XHigh),
        developer_instructions: None,
    })
}
