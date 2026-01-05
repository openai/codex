use chrono::Utc;
use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use http::HeaderMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::TryLockError;
use tracing::error;

use super::cache;
use super::cache::ModelsCache;
use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::auth::AuthManager;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::error::Result as CoreResult;
use crate::features::Feature;
use crate::model_provider_info::ModelProviderInfo;
use crate::models_manager::model_info;
use crate::models_manager::model_presets::builtin_model_presets;

const MODEL_CACHE_FILE: &str = "models_cache.json";
const DEFAULT_MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
const OPENAI_DEFAULT_API_MODEL: &str = "gpt-5.1-codex-max";
const OPENAI_DEFAULT_CHATGPT_MODEL: &str = "gpt-5.2-codex";
const CODEX_AUTO_BALANCED_MODEL: &str = "codex-auto-balanced";

/// Coordinates remote model discovery plus cached metadata on disk.
#[derive(Debug)]
pub struct ModelsManager {
    // todo(aibrahim) merge available_models and model family creation into one struct
    local_models: Vec<ModelPreset>,
    remote_models: RwLock<Vec<ModelInfo>>,
    auth_manager: Arc<AuthManager>,
    etag: RwLock<Option<String>>,
    codex_home: PathBuf,
    cache_ttl: Duration,
    provider: ModelProviderInfo,
}

impl ModelsManager {
    /// Construct a manager scoped to the provided `AuthManager`.
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        let codex_home = auth_manager.codex_home().to_path_buf();
        Self {
            local_models: builtin_model_presets(auth_manager.get_auth_mode()),
            remote_models: RwLock::new(Self::load_remote_models_from_file().unwrap_or_default()),
            auth_manager,
            etag: RwLock::new(None),
            codex_home,
            cache_ttl: DEFAULT_MODEL_CACHE_TTL,
            provider: ModelProviderInfo::create_openai_provider(),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Construct a manager scoped to the provided `AuthManager` with a specific provider. Used for integration tests.
    pub fn with_provider(auth_manager: Arc<AuthManager>, provider: ModelProviderInfo) -> Self {
        let codex_home = auth_manager.codex_home().to_path_buf();
        Self {
            local_models: builtin_model_presets(auth_manager.get_auth_mode()),
            remote_models: RwLock::new(Self::load_remote_models_from_file().unwrap_or_default()),
            auth_manager,
            etag: RwLock::new(None),
            codex_home,
            cache_ttl: DEFAULT_MODEL_CACHE_TTL,
            provider,
        }
    }

    /// Fetch the latest remote models, using the on-disk cache when still fresh.
    pub async fn refresh_available_models_with_cache(&self, config: &Config) -> CoreResult<()> {
        if !config.features.enabled(Feature::RemoteModels)
            || self.auth_manager.get_auth_mode() == Some(AuthMode::ApiKey)
        {
            return Ok(());
        }
        if self.try_load_cache().await {
            return Ok(());
        }
        self.refresh_available_models_no_cache(config.features.enabled(Feature::RemoteModels))
            .await
    }

    pub(crate) async fn refresh_available_models_no_cache(
        &self,
        remote_models_feature: bool,
    ) -> CoreResult<()> {
        if !remote_models_feature || self.auth_manager.get_auth_mode() == Some(AuthMode::ApiKey) {
            return Ok(());
        }
        let auth = self.auth_manager.auth();
        let api_provider = self.provider.to_api_provider(Some(AuthMode::ChatGPT))?;
        let api_auth = auth_provider_from_auth(auth.clone(), &self.provider).await?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let client_version = format_client_version_to_whole();
        let (models, etag) = client
            .list_models(&client_version, HeaderMap::new())
            .await
            .map_err(map_api_error)?;

        self.apply_remote_models(models.clone()).await;
        *self.etag.write().await = etag.clone();
        self.persist_cache(&models, etag).await;
        Ok(())
    }

    pub async fn list_models(&self, config: &Config) -> Vec<ModelPreset> {
        if let Err(err) = self.refresh_available_models_with_cache(config).await {
            error!("failed to refresh available models: {err}");
        }
        let remote_models = self.remote_models(config).await;
        self.build_available_models(remote_models)
    }

    pub fn try_list_models(&self, config: &Config) -> Result<Vec<ModelPreset>, TryLockError> {
        let remote_models = self.try_get_remote_models(config)?;
        Ok(self.build_available_models(remote_models))
    }

    fn find_model_info_for_slug(slug: &str) -> ModelInfo {
        model_info::find_model_info_for_slug(slug)
    }

    /// Look up the requested model metadata while applying remote metadata overrides.
    pub async fn construct_model_info(&self, model: &str, config: &Config) -> ModelInfo {
        let remote = self
            .remote_models(config)
            .await
            .into_iter()
            .find(|m| m.slug == model);
        let model = Self::find_model_info_for_slug(model);
        model_info::with_config_overrides(model_info::merge_remote_overrides(model, remote), config)
    }

    pub async fn get_model(&self, model: &Option<String>, config: &Config) -> String {
        if let Some(model) = model.as_ref() {
            return model.to_string();
        }
        if let Err(err) = self.refresh_available_models_with_cache(config).await {
            error!("failed to refresh available models: {err}");
        }
        // if codex-auto-balanced exists & signed in with chatgpt mode, return it, otherwise return the default model
        let auth_mode = self.auth_manager.get_auth_mode();
        let remote_models = self.remote_models(config).await;
        if auth_mode == Some(AuthMode::ChatGPT)
            && self
                .build_available_models(remote_models)
                .iter()
                .any(|m| m.model == CODEX_AUTO_BALANCED_MODEL)
        {
            return CODEX_AUTO_BALANCED_MODEL.to_string();
        } else if auth_mode == Some(AuthMode::ChatGPT) {
            return OPENAI_DEFAULT_CHATGPT_MODEL.to_string();
        }
        OPENAI_DEFAULT_API_MODEL.to_string()
    }
    pub async fn refresh_if_new_etag(&self, etag: String, remote_models_feature: bool) {
        let current_etag = self.get_etag().await;
        if current_etag.clone().is_some() && current_etag.as_deref() == Some(etag.as_str()) {
            return;
        }
        if let Err(err) = self
            .refresh_available_models_no_cache(remote_models_feature)
            .await
        {
            error!("failed to refresh available models: {err}");
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn get_model_offline(model: Option<&str>) -> String {
        model.unwrap_or(OPENAI_DEFAULT_CHATGPT_MODEL).to_string()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Offline helper that builds a `ModelInfo` without consulting remote state.
    pub fn construct_model_info_offline(model: &str, config: &Config) -> ModelInfo {
        model_info::with_config_overrides(Self::find_model_info_for_slug(model), config)
    }

    async fn get_etag(&self) -> Option<String> {
        self.etag.read().await.clone()
    }

    /// Replace the cached remote models and rebuild the derived presets list.
    async fn apply_remote_models(&self, models: Vec<ModelInfo>) {
        *self.remote_models.write().await = models;
    }

    fn load_remote_models_from_file() -> Result<Vec<ModelInfo>, std::io::Error> {
        let file_contents = include_str!("../../models.json");
        let response: ModelsResponse = serde_json::from_str(file_contents)?;
        Ok(response.models)
    }

    /// Attempt to satisfy the refresh from the cache when it matches the provider and TTL.
    async fn try_load_cache(&self) -> bool {
        // todo(aibrahim): think if we should store fetched_at in ModelsManager so we don't always need to read the disk
        let cache_path = self.cache_path();
        let cache = match cache::load_cache(&cache_path).await {
            Ok(cache) => cache,
            Err(err) => {
                error!("failed to load models cache: {err}");
                return false;
            }
        };
        let cache = match cache {
            Some(cache) => cache,
            None => return false,
        };
        if !cache.is_fresh(self.cache_ttl) {
            return false;
        }
        let models = cache.models.clone();
        *self.etag.write().await = cache.etag.clone();
        self.apply_remote_models(models.clone()).await;
        true
    }

    /// Serialize the latest fetch to disk for reuse across future processes.
    async fn persist_cache(&self, models: &[ModelInfo], etag: Option<String>) {
        let cache = ModelsCache {
            fetched_at: Utc::now(),
            etag,
            models: models.to_vec(),
        };
        let cache_path = self.cache_path();
        if let Err(err) = cache::save_cache(&cache_path, &cache).await {
            error!("failed to write models cache: {err}");
        }
    }

    /// Merge remote model metadata into picker-ready presets, preserving existing entries.
    fn build_available_models(&self, mut remote_models: Vec<ModelInfo>) -> Vec<ModelPreset> {
        remote_models.sort_by(|a, b| a.priority.cmp(&b.priority));

        let remote_presets: Vec<ModelPreset> = remote_models.into_iter().map(Into::into).collect();
        let existing_presets = self.local_models.clone();
        let mut merged_presets = Self::merge_presets(remote_presets, existing_presets);
        merged_presets = self.filter_visible_models(merged_presets);

        let has_default = merged_presets.iter().any(|preset| preset.is_default);
        if let Some(default) = merged_presets.first_mut()
            && !has_default
        {
            default.is_default = true;
        }

        merged_presets
    }

    fn filter_visible_models(&self, models: Vec<ModelPreset>) -> Vec<ModelPreset> {
        let chatgpt_mode = self.auth_manager.get_auth_mode() == Some(AuthMode::ChatGPT);
        models
            .into_iter()
            .filter(|model| model.show_in_picker && (chatgpt_mode || model.supported_in_api))
            .collect()
    }

    fn merge_presets(
        remote_presets: Vec<ModelPreset>,
        existing_presets: Vec<ModelPreset>,
    ) -> Vec<ModelPreset> {
        if remote_presets.is_empty() {
            return existing_presets;
        }

        let remote_slugs: HashSet<&str> = remote_presets
            .iter()
            .map(|preset| preset.model.as_str())
            .collect();

        let mut merged_presets = remote_presets.clone();
        for mut preset in existing_presets {
            if remote_slugs.contains(preset.model.as_str()) {
                continue;
            }
            preset.is_default = false;
            merged_presets.push(preset);
        }

        merged_presets
    }

    async fn remote_models(&self, config: &Config) -> Vec<ModelInfo> {
        if config.features.enabled(Feature::RemoteModels) {
            self.remote_models.read().await.clone()
        } else {
            Vec::new()
        }
    }

    fn try_get_remote_models(&self, config: &Config) -> Result<Vec<ModelInfo>, TryLockError> {
        if config.features.enabled(Feature::RemoteModels) {
            Ok(self.remote_models.try_read()?.clone())
        } else {
            Ok(Vec::new())
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.codex_home.join(MODEL_CACHE_FILE)
    }
}

/// Convert a client version string to a whole version string (e.g. "1.2.3-alpha.4" -> "1.2.3")
fn format_client_version_to_whole() -> String {
    format!(
        "{}.{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    )
}

#[cfg(test)]
mod tests {
    use super::cache::ModelsCache;
    use super::*;
    use crate::CodexAuth;
    use crate::auth::AuthCredentialsStoreMode;
    use crate::config::ConfigBuilder;
    use crate::features::Feature;
    use crate::model_provider_info::WireApi;
    use codex_protocol::openai_models::ModelsResponse;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::tempdir;

    fn remote_model(slug: &str, display: &str, priority: i32) -> ModelInfo {
        remote_model_with_visibility(slug, display, priority, "list")
    }

    fn remote_model_with_visibility(
        slug: &str,
        display: &str,
        priority: i32,
        visibility: &str,
    ) -> ModelInfo {
        serde_json::from_value(json!({
            "slug": slug,
            "display_name": display,
            "description": format!("{display} desc"),
            "default_reasoning_level": "medium",
            "supported_reasoning_levels": [{"effort": "low", "description": "low"}, {"effort": "medium", "description": "medium"}],
            "shell_type": "shell_command",
            "visibility": visibility,
            "minimal_client_version": [0, 1, 0],
            "supported_in_api": true,
            "priority": priority,
            "upgrade": null,
            "base_instructions": "base instructions",
            "supports_reasoning_summaries": false,
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "truncation_policy": {"mode": "bytes", "limit": 10_000},
            "supports_parallel_tool_calls": false,
            "context_window": 272_000,
            "experimental_supported_tools": [],
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

    #[test]
    fn build_available_models_sorts_and_marks_default() {
        let codex_home = tempdir().expect("temp dir");
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let provider = provider_for("http://example.test".to_string());
        let manager = ModelsManager::with_provider(auth_manager, provider);

        let remote_models = vec![
            remote_model("priority-low", "Low", 1),
            remote_model("priority-high", "High", 0),
        ];
        let available = manager.build_available_models(remote_models);

        let high_idx = available
            .iter()
            .position(|model| model.model == "priority-high")
            .expect("priority-high should be listed");
        let low_idx = available
            .iter()
            .position(|model| model.model == "priority-low")
            .expect("priority-low should be listed");
        assert!(
            high_idx < low_idx,
            "higher priority should be listed before lower priority"
        );
        assert!(
            available[high_idx].is_default,
            "highest priority should be default"
        );
        assert!(!available[low_idx].is_default);

        // Keep `codex_home` alive for the duration of the test even though it is
        // unused; some `AuthManager` implementations may reference it.
        drop(codex_home);
    }

    #[tokio::test]
    async fn try_load_cache_uses_cache_when_fresh() {
        let codex_home = tempdir().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("load default test config");
        config.features.enable(Feature::RemoteModels);

        let auth_manager = Arc::new(AuthManager::new(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ));
        let provider = provider_for("http://example.test".to_string());
        let manager = ModelsManager::with_provider(auth_manager, provider);

        let remote_models = vec![remote_model("cached", "Cached", 5)];
        manager
            .persist_cache(&remote_models, Some("etag-1".to_string()))
            .await;

        assert!(
            manager.try_load_cache().await,
            "fresh cache should be loaded"
        );
        assert_eq!(
            manager.remote_models(&config).await,
            remote_models,
            "remote cache should store fetched models"
        );
        assert_eq!(
            manager.get_etag().await.as_deref(),
            Some("etag-1"),
            "etag should be restored from cache"
        );
    }

    #[tokio::test]
    async fn try_load_cache_returns_false_when_cache_stale() {
        let codex_home = tempdir().expect("temp dir");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("load default test config");
        config.features.enable(Feature::RemoteModels);

        let auth_manager = Arc::new(AuthManager::new(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ));
        let provider = provider_for("http://example.test".to_string());
        let mut manager = ModelsManager::with_provider(auth_manager, provider);
        manager.cache_ttl = Duration::from_secs(10);
        manager.apply_remote_models(Vec::new()).await;

        let cache_path = codex_home.path().join(MODEL_CACHE_FILE);
        let cache = ModelsCache {
            fetched_at: Utc::now() - chrono::Duration::hours(1),
            etag: Some("etag-old".to_string()),
            models: vec![remote_model("stale", "Stale", 1)],
        };
        cache::save_cache(&cache_path, &cache)
            .await
            .expect("cache write succeeds");

        assert!(
            !manager.try_load_cache().await,
            "stale cache should not load"
        );
        assert_eq!(
            manager.remote_models(&config).await,
            Vec::<ModelInfo>::new(),
            "stale cache should not update remote models"
        );
    }

    #[test]
    fn build_available_models_drops_removed_remote_models() {
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let provider = provider_for("http://example.test".to_string());
        let mut manager = ModelsManager::with_provider(auth_manager, provider);
        manager.local_models = Vec::new();

        let available0 = manager.build_available_models(vec![remote_model("remote-old", "Old", 1)]);
        assert!(
            available0.iter().any(|preset| preset.model == "remote-old"),
            "initial remote model should be listed"
        );

        let available1 = manager.build_available_models(vec![remote_model("remote-new", "New", 1)]);
        assert!(
            available1.iter().any(|preset| preset.model == "remote-new"),
            "new remote model should be listed"
        );
        assert!(
            !available1.iter().any(|preset| preset.model == "remote-old"),
            "removed remote model should not be listed"
        );
    }

    #[test]
    fn build_available_models_picks_default_after_hiding_hidden_models() {
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let provider = provider_for("http://example.test".to_string());
        let mut manager = ModelsManager::with_provider(auth_manager, provider);
        manager.local_models = Vec::new();

        let hidden_model = remote_model_with_visibility("hidden", "Hidden", 0, "hide");
        let visible_model = remote_model_with_visibility("visible", "Visible", 1, "list");

        let mut expected = ModelPreset::from(visible_model.clone());
        expected.is_default = true;

        let available = manager.build_available_models(vec![hidden_model, visible_model]);

        assert_eq!(available, vec![expected]);
    }

    #[test]
    fn bundled_models_json_roundtrips() {
        let file_contents = include_str!("../../models.json");
        let response: ModelsResponse =
            serde_json::from_str(file_contents).expect("bundled models.json should deserialize");

        let serialized =
            serde_json::to_string(&response).expect("bundled models.json should serialize");
        let roundtripped: ModelsResponse =
            serde_json::from_str(&serialized).expect("serialized models.json should deserialize");

        assert_eq!(
            response, roundtripped,
            "bundled models.json should round trip through serde"
        );
        assert!(
            !response.models.is_empty(),
            "bundled models.json should contain at least one model"
        );
    }
}
