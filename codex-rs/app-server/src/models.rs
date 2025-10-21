use codex_app_server_protocol::Model;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_common::model_presets::ModelPreset;
use codex_common::model_presets::builtin_model_presets;
use codex_protocol::config_types::ReasoningEffort;

const DEFAULT_MODEL_SLUG: &str = "gpt-5-codex";
pub const DEFAULT_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Medium;

pub fn codex_models() -> Vec<Model> {
    let presets = builtin_model_presets(None);

    vec![
        Model {
            id: DEFAULT_MODEL_SLUG.to_string(),
            slug: DEFAULT_MODEL_SLUG.to_string(),
            display_name: "GPT-5 Codex".to_string(),
            description: "Specialized GPT-5 variant optimized for Codex.".to_string(),
            supported_reasoning_efforts: reasoning_effort_options(&presets, DEFAULT_MODEL_SLUG),
            default_reasoning_effort: DEFAULT_REASONING_EFFORT,
            is_default: true,
        },
        Model {
            id: "gpt-5".to_string(),
            slug: "gpt-5".to_string(),
            display_name: "GPT-5".to_string(),
            description: "General-purpose GPT-5 model.".to_string(),
            supported_reasoning_efforts: reasoning_effort_options(&presets, "gpt-5"),
            default_reasoning_effort: DEFAULT_REASONING_EFFORT,
            is_default: false,
        },
    ]
}

fn reasoning_effort_options(
    presets: &[ModelPreset],
    model_slug: &str,
) -> Vec<ReasoningEffortOption> {
    presets
        .iter()
        .filter(|preset| preset.model == model_slug)
        .filter_map(|preset| {
            preset.effort.map(|effort| ReasoningEffortOption {
                reasoning_effort: effort,
                description: preset.description.to_string(),
            })
        })
        .collect()
}
