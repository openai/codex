use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;

/// Legacy notice keys kept for config compatibility with older migration prompts.
///
/// Hardcoded model presets were removed; model listings are now derived from the active catalog.
pub const HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG: &str = "hide_gpt5_1_migration_prompt";
pub const HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG: &str =
    "hide_gpt-5.1-codex-max_migration_prompt";

/// Removes the gated Ultra effort from model presets exposed to clients or tools.
pub fn hide_ultra_reasoning_effort(models: &mut [ModelPreset]) {
    for model in models {
        model
            .supported_reasoning_efforts
            .retain(|preset| preset.effort != ReasoningEffort::Ultra);
        if model.default_reasoning_effort == ReasoningEffort::Ultra {
            model.default_reasoning_effort = model
                .supported_reasoning_efforts
                .last()
                .map(|preset| preset.effort.clone())
                .unwrap_or(ReasoningEffort::None);
        }
    }
}
