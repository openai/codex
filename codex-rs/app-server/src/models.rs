use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::Model;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_common::model_presets::ModelPreset;
use codex_common::model_presets::ReasoningEffortPreset;
use codex_common::model_presets::builtin_model_presets;

pub fn supported_models(auth_mode: Option<AuthMode>) -> Vec<Model> {
    builtin_model_presets(auth_mode)
        .into_iter()
        .map(model_from_preset)
        .collect()
}

fn model_from_preset(preset: ModelPreset) -> Model {
    Model {
        id: preset.id.to_string(),
        model: preset.model.to_string(),
        display_name: preset.display_name.to_string(),
        description: preset.description.to_string(),
        supported_reasoning_efforts: reasoning_efforts_from_preset(
            preset.supported_reasoning_efforts,
        ),
        default_reasoning_effort: preset.default_reasoning_effort,
        is_default: preset.is_default,
    }
}

fn reasoning_efforts_from_preset(
    efforts: &'static [ReasoningEffortPreset],
) -> Vec<ReasoningEffortOption> {
    efforts
        .iter()
        .map(|preset| ReasoningEffortOption {
            reasoning_effort: preset.effort,
            description: preset.description.to_string(),
        })
        .collect()
}
