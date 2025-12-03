use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelPreset;

use crate::openai_models::model_presets::builtin_model_presets;

pub struct ModelsManager {
    pub available_models: Vec<ModelPreset>,
    pub etag: String,
    pub auth_mode: Option<AuthMode>,
}

impl ModelsManager {
    pub fn new(auth_mode: Option<AuthMode>) -> Self {
        Self {
            available_models: builtin_model_presets(auth_mode),
            etag: String::new(),
            auth_mode,
        }
    }

    pub fn refresh_available_models(&self) -> Vec<ModelPreset> {
        builtin_model_presets(self.auth_mode)
    }
}
