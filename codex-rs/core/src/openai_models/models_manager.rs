use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelPreset;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::openai_models::model_family::ModelFamily;
use crate::openai_models::model_family::find_family_for_model;
use crate::openai_models::model_presets::builtin_model_presets;

#[derive(Debug)]
pub struct ModelsManager {
    pub available_models: RwLock<Vec<ModelPreset>>,
    pub etag: String,
    pub auth_mode: Option<AuthMode>,
}

impl ModelsManager {
    pub fn new(auth_mode: Option<AuthMode>) -> Self {
        Self {
            available_models: RwLock::new(builtin_model_presets(auth_mode)),
            etag: String::new(),
            auth_mode,
        }
    }

    pub async fn refresh_available_models(&self) {
        let models = builtin_model_presets(self.auth_mode);
        *self.available_models.write().await = models;
    }

    pub fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        find_family_for_model(model).with_config_overrides(config)
    }
}
