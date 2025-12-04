use std::sync::Arc;

use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use http::HeaderMap;
use tokio::sync::RwLock;

use crate::AuthManager;
use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::auth::CodexAuth;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::error::Result;
use crate::model_provider_info::ModelProviderInfo;
use crate::openai_models::model_family::ModelFamily;
use crate::openai_models::model_family::find_family_for_model;
use crate::openai_models::model_presets::builtin_model_presets;

#[derive(Debug)]
pub struct ModelsManager {
    pub available_models: RwLock<Vec<ModelPreset>>,
    pub etag: String,
    pub auth_manager: Arc<AuthManager>,
}

impl ModelsManager {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            available_models: RwLock::new(builtin_model_presets(auth_manager.get_auth_mode())),
            etag: String::new(),
            auth_manager,
        }
    }

    pub async fn refresh_available_models(&self) {
        let models = builtin_model_presets(self.auth_manager.get_auth_mode());
        *self.available_models.write().await = models;
    }

    pub fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        find_family_for_model(model).with_config_overrides(config)
    }

    pub async fn fetch_models_from_api(
        &self,
        provider: &ModelProviderInfo,
    ) -> Result<Vec<ModelInfo>> {
        let api_provider = provider.to_api_provider(self.auth_manager.get_auth_mode())?;
        let api_auth = auth_provider_from_auth(self.auth_manager.auth(), provider).await?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let response = client
            .list_models(env!("CARGO_PKG_VERSION"), HeaderMap::new())
            .await
            .map_err(map_api_error)?;

        Ok(response.models)
    }
}
