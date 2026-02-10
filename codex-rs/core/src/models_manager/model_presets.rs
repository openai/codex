use crate::auth::AuthMode;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;

pub const HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG: &str = "hide_gpt5_1_migration_prompt";
pub const HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG: &str =
    "hide_gpt-5.1-codex-max_migration_prompt";

#[cfg(any(test, feature = "test-support"))]
use once_cell::sync::Lazy;

fn builtin_model_presets_from_models_json() -> Vec<ModelPreset> {
    match serde_json::from_str::<ModelsResponse>(include_str!("../../models.json")) {
        Ok(models_response) => models_response
            .models
            .into_iter()
            .map(ModelPreset::from)
            .collect(),
        Err(..) => vec![],
    }
}

pub(super) fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    builtin_model_presets_from_models_json()
}

#[cfg(any(test, feature = "test-support"))]
static BUILTIN_PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(builtin_model_presets_from_models_json);

pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &BUILTIN_PRESETS
}
