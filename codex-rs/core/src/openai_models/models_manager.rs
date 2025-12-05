use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use http::HeaderMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::auth::AuthManager;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::error::Result as CoreResult;
use crate::model_provider_info::ModelProviderInfo;
use crate::openai_models::model_family::ModelFamily;
use crate::openai_models::model_family::find_family_for_model;
use crate::openai_models::model_presets::builtin_model_presets;

#[derive(Debug)]
pub struct ModelsManager {
    // todo(aibrahim) merge available_models and model family creation into one struct
    pub available_models: RwLock<Vec<ModelPreset>>,
    pub remote_models: RwLock<Vec<ModelInfo>>,
    pub etag: String,
    pub auth_manager: Arc<AuthManager>,
}

impl ModelsManager {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            available_models: RwLock::new(builtin_model_presets(auth_manager.get_auth_mode())),
            remote_models: RwLock::new(Vec::new()),
            etag: String::new(),
            auth_manager,
        }
    }

    // do not use this function yet. It's work in progress.
    pub async fn refresh_available_models(
        &self,
        provider: &ModelProviderInfo,
    ) -> CoreResult<Vec<ModelInfo>> {
        let auth = self.auth_manager.auth();
        let api_provider = provider.to_api_provider(auth.as_ref().map(|auth| auth.mode))?;
        let api_auth = auth_provider_from_auth(auth.clone(), provider).await?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let response = client
            .list_models(env!("CARGO_PKG_VERSION"), HeaderMap::new())
            .await
            .map_err(map_api_error)?;

        let models = response.models;
        *self.remote_models.write().await = models.clone();
        {
            let mut available_models_guard = self.available_models.write().await;
            *available_models_guard = self.build_available_models().await;
        }
        Ok(models)
    }

    pub async fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        find_family_for_model(model)
            .with_config_overrides(config)
            .with_remote_overrides(self.remote_models.read().await.clone())
    }

    pub async fn build_available_models(&self) -> Vec<ModelPreset> {
        let mut available_models = self.remote_models.read().await.clone();
        available_models.sort_by(|a, b| b.priority.cmp(&a.priority));
        let mut model_presets: Vec<ModelPreset> =
            available_models.into_iter().map(Into::into).collect();
        if let Some(default) = model_presets.first_mut() {
            default.is_default = true;
        }
        model_presets
    }
}
