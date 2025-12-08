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

    pub async fn refresh_available_models(
        &self,
        provider: &ModelProviderInfo,
    ) -> CoreResult<Vec<ModelInfo>> {
        let auth = self.auth_manager.auth();
        let api_provider = provider.to_api_provider(auth.as_ref().map(|auth| auth.mode))?;
        let api_auth = auth_provider_from_auth(auth.clone(), provider).await?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let mut client_version = env!("CARGO_PKG_VERSION");
        if client_version == "0.0.0" {
            client_version = "99.99.99";
        }
        let response = client
            .list_models(client_version, HeaderMap::new())
            .await
            .map_err(map_api_error)?;

        let models = response.models;
        *self.remote_models.write().await = models.clone();
        let available_models = self.build_available_models().await;
        {
            let mut available_models_guard = self.available_models.write().await;
            *available_models_guard = available_models;
        }
        Ok(models)
    }

    pub async fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        find_family_for_model(model)
            .with_config_overrides(config)
            .with_remote_overrides(self.remote_models.read().await.clone())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn construct_model_family_offline(model: &str, config: &Config) -> ModelFamily {
        find_family_for_model(model).with_config_overrides(config)
    }

    async fn build_available_models(&self) -> Vec<ModelPreset> {
        let mut available_models = self.remote_models.read().await.clone();
        available_models.sort_by(|a, b| b.priority.cmp(&a.priority));
        let mut model_presets: Vec<ModelPreset> = available_models
            .into_iter()
            .map(Into::into)
            .filter(|preset: &ModelPreset| preset.show_in_picker)
            .collect();
        if let Some(default) = model_presets.first_mut() {
            default.is_default = true;
        }
        model_presets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodexAuth;
    use crate::model_provider_info::WireApi;
    use codex_protocol::openai_models::ModelsResponse;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    fn remote_model(slug: &str, display: &str, priority: i32) -> ModelInfo {
        serde_json::from_value(json!({
            "slug": slug,
            "display_name": display,
            "description": format!("{display} desc"),
            "default_reasoning_level": "medium",
            "supported_reasoning_levels": [{"effort": "low", "description": "low"}, {"effort": "medium", "description": "medium"}],
            "shell_type": "shell_command",
            "visibility": "list",
            "minimal_client_version": [0, 1, 0],
            "supported_in_api": true,
            "priority": priority,
            "upgrade": null,
        }))
        .expect("valid model")
    }

    fn provider_for(base_url: String) -> ModelProviderInfo {
        ModelProviderInfo {
            name: "mock".into(),
            base_url: Some(base_url),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: Some(5_000),
            requires_openai_auth: false,
        }
    }

    #[tokio::test]
    async fn refresh_available_models_sorts_and_marks_default() {
        let server = MockServer::start().await;
        let remote_models = vec![
            remote_model("priority-low", "Low", 1),
            remote_model("priority-high", "High", 10),
        ];
        let response = ModelsResponse {
            models: remote_models.clone(),
        };
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(&response),
            )
            .expect(1)
            .mount(&server)
            .await;

        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let manager = ModelsManager::new(auth_manager);
        let provider = provider_for(server.uri());

        let returned = manager
            .refresh_available_models(&provider)
            .await
            .expect("refresh succeeds");

        assert_eq!(returned, remote_models);
        let cached_remote = manager.remote_models.read().await.clone();
        assert_eq!(cached_remote, remote_models);

        let available = manager.available_models.read().await.clone();
        assert_eq!(available.len(), 2);
        assert_eq!(available[0].model, "priority-high");
        assert!(
            available[0].is_default,
            "highest priority should be default"
        );
        assert_eq!(available[1].model, "priority-low");
        assert!(!available[1].is_default);
    }
}
