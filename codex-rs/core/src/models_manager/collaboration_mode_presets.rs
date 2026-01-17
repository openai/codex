use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;

pub(super) fn builtin_collaboration_mode_presets(
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
) -> Vec<CollaborationMode> {
    let settings = Settings {
        model,
        reasoning_effort,
        developer_instructions: None,
    };
    vec![
        CollaborationMode::Plan(settings.clone()),
        CollaborationMode::Collaborate(settings.clone()),
        CollaborationMode::Execute(settings),
    ]
}
