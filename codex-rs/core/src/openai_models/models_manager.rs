use chrono::Utc;
use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use http::HeaderMap;
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
use crate::openai_models::model_family::ModelFamily;
use crate::openai_models::model_presets::builtin_model_presets;

const MODEL_CACHE_FILE: &str = "models_cache.json";
const DEFAULT_MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
const OPENAI_DEFAULT_MODEL: &str = "gpt-5.1-codex-max";
const CODEX_AUTO_BALANCED_MODEL: &str = "codex-auto-balanced";

/// Coordinates remote model discovery plus cached metadata on disk.
#[derive(Debug)]
pub struct ModelsManager {
    // todo(aibrahim) merge available_models and model family creation into one struct
    available_models: RwLock<Vec<ModelPreset>>,
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
            available_models: RwLock::new(builtin_model_presets(auth_manager.get_auth_mode())),
            remote_models: RwLock::new(Vec::new()),
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
            available_models: RwLock::new(builtin_model_presets(auth_manager.get_auth_mode())),
            remote_models: RwLock::new(Vec::new()),
            auth_manager,
            etag: RwLock::new(None),
            codex_home,
            cache_ttl: DEFAULT_MODEL_CACHE_TTL,
            provider,
        }
    }

    /// Fetch the latest remote models, using the on-disk cache when still fresh.
    pub async fn refresh_available_models(&self, config: &Config) -> CoreResult<()> {
        if !config.features.enabled(Feature::RemoteModels) {
            return Ok(());
        }
        if self.try_load_cache().await {
            return Ok(());
        }

        let auth = self.auth_manager.auth();
        let api_provider = self.provider.to_api_provider(Some(AuthMode::ChatGPT))?;
        let api_auth = auth_provider_from_auth(auth.clone(), &self.provider).await?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let client_version = format_client_version_to_whole();
        let ModelsResponse { models, etag } = client
            .list_models(&client_version, HeaderMap::new())
            .await
            .map_err(map_api_error)?;

        let etag = (!etag.is_empty()).then_some(etag);

        self.apply_remote_models(models.clone()).await;
        *self.etag.write().await = etag.clone();
        self.persist_cache(&models, etag).await;
        Ok(())
    }

    pub async fn list_models(&self, config: &Config) -> Vec<ModelPreset> {
        if let Err(err) = self.refresh_available_models(config).await {
            error!("failed to refresh available models: {err}");
        }
        self.available_models.read().await.clone()
    }

    pub fn try_list_models(&self) -> Result<Vec<ModelPreset>, TryLockError> {
        self.available_models
            .try_read()
            .map(|models| models.clone())
    }

    fn find_family_for_model(slug: &str) -> ModelFamily {
        super::model_family::find_family_for_model(slug)
    }

    /// Look up the requested model family while applying remote metadata overrides.
    pub async fn construct_model_family(&self, model: &str, config: &Config) -> ModelFamily {
        Self::find_family_for_model(model)
            .with_config_overrides(config)
            .with_remote_overrides(self.remote_models.read().await.clone())
    }

    pub async fn get_model(&self, model: &Option<String>, config: &Config) -> String {
        if let Some(model) = model.as_ref() {
            return model.to_string();
        }
        if let Err(err) = self.refresh_available_models(config).await {
            error!("failed to refresh available models: {err}");
        }
        // if codex-auto-balanced exists & signed in with chatgpt mode, return it, otherwise return the default model
        let auth_mode = self.auth_manager.get_auth_mode();
        if auth_mode == Some(AuthMode::ChatGPT)
            && self
                .available_models
                .read()
                .await
                .iter()
                .any(|m| m.model == CODEX_AUTO_BALANCED_MODEL)
        {
            return CODEX_AUTO_BALANCED_MODEL.to_string();
        }
        OPENAI_DEFAULT_MODEL.to_string()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn get_model_offline(model: Option<&str>) -> String {
        model.unwrap_or(OPENAI_DEFAULT_MODEL).to_string()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Offline helper that builds a `ModelFamily` without consulting remote state.
    pub fn construct_model_family_offline(model: &str, config: &Config) -> ModelFamily {
        Self::find_family_for_model(model).with_config_overrides(config)
    }

    /// Replace the cached remote models and rebuild the derived presets list.
    async fn apply_remote_models(&self, models: Vec<ModelInfo>) {
        *self.remote_models.write().await = models;
        self.build_available_models().await;
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

    /// Convert remote model metadata into picker-ready presets, marking defaults.
    async fn build_available_models(&self) {
        let mut available_models = self.remote_models.read().await.clone();
        available_models.sort_by(|a, b| a.priority.cmp(&b.priority));
        let mut model_presets: Vec<ModelPreset> = available_models
            .into_iter()
            .map(Into::into)
            .filter(|preset: &ModelPreset| preset.show_in_picker)
            .collect();
        if let Some(default) = model_presets.first_mut() {
            default.is_default = true;
        }
        {
            let mut available_models_guard = self.available_models.write().await;
            *available_models_guard = model_presets;
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.codex_home.join(MODEL_CACHE_FILE)
    }
}

/// Convert a client version string to a whole version string (e.g. "1.2.3-alpha.4" -> "1.2.3")
fn format_client_version_to_whole() -> String {
    format_client_version_from_parts(
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    )
}

fn format_client_version_from_parts(major: &str, minor: &str, patch: &str) -> String {
    const DEV_VERSION: &str = "0.0.0";
    const FALLBACK_VERSION: &str = "99.99.99";

    let normalized = format!("{major}.{minor}.{patch}");

    if normalized == DEV_VERSION {
        FALLBACK_VERSION.to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::cache::ModelsCache;
    use super::*;
    use crate::CodexAuth;
    use crate::auth::AuthCredentialsStoreMode;
    use crate::config::Config;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::features::Feature;
    use crate::model_provider_info::WireApi;
    use codex_protocol::openai_models::ModelsResponse;
    use core_test_support::responses::mount_models_once;
    use serde_json::json;
    use tempfile::tempdir;
    use wiremock::MockServer;

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
            "base_instructions": null,
            "supports_reasoning_summaries": false,
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "truncation_policy": {"mode": "bytes", "limit": 10_000},
            "supports_parallel_tool_calls": false,
            "context_window": null,
            "reasoning_summary_format": "none",
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

    #[tokio::test]
    async fn refresh_available_models_sorts_and_marks_default() {
        let server = MockServer::start().await;
        let remote_models = vec![
            remote_model("priority-low", "Low", 1),
            remote_model("priority-high", "High", 0),
        ];
        let models_mock = mount_models_once(
            &server,
            ModelsResponse {
                models: remote_models.clone(),
                etag: String::new(),
            },
        )
        .await;

        let codex_home = tempdir().expect("temp dir");
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load default test config");
        config.features.enable(Feature::RemoteModels);
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let provider = provider_for(server.uri());
        let manager = ModelsManager::with_provider(auth_manager, provider);

        manager
            .refresh_available_models(&config)
            .await
            .expect("refresh succeeds");
        let cached_remote = manager.remote_models.read().await.clone();
        assert_eq!(cached_remote, remote_models);

        let available = manager.list_models(&config).await;
        assert_eq!(available.len(), 2);
        assert_eq!(available[0].model, "priority-high");
        assert!(
            available[0].is_default,
            "highest priority should be default"
        );
        assert_eq!(available[1].model, "priority-low");
        assert!(!available[1].is_default);
        assert_eq!(
            models_mock.requests().len(),
            1,
            "expected a single /models request"
        );
    }

    #[tokio::test]
    async fn refresh_available_models_uses_cache_when_fresh() {
        let server = MockServer::start().await;
        let remote_models = vec![remote_model("cached", "Cached", 5)];
        let models_mock = mount_models_once(
            &server,
            ModelsResponse {
                models: remote_models.clone(),
                etag: String::new(),
            },
        )
        .await;

        let codex_home = tempdir().expect("temp dir");
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load default test config");
        config.features.enable(Feature::RemoteModels);
        let auth_manager = Arc::new(AuthManager::new(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ));
        let provider = provider_for(server.uri());
        let manager = ModelsManager::with_provider(auth_manager, provider);

        manager
            .refresh_available_models(&config)
            .await
            .expect("first refresh succeeds");
        assert_eq!(
            *manager.remote_models.read().await,
            remote_models,
            "remote cache should store fetched models"
        );

        // Second call should read from cache and avoid the network.
        manager
            .refresh_available_models(&config)
            .await
            .expect("cached refresh succeeds");
        assert_eq!(
            *manager.remote_models.read().await,
            remote_models,
            "cache path should not mutate stored models"
        );
        assert_eq!(
            models_mock.requests().len(),
            1,
            "cache hit should avoid a second /models request"
        );
    }

    #[tokio::test]
    async fn refresh_available_models_refetches_when_cache_stale() {
        let server = MockServer::start().await;
        let initial_models = vec![remote_model("stale", "Stale", 1)];
        let initial_mock = mount_models_once(
            &server,
            ModelsResponse {
                models: initial_models.clone(),
                etag: String::new(),
            },
        )
        .await;

        let codex_home = tempdir().expect("temp dir");
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load default test config");
        config.features.enable(Feature::RemoteModels);
        let auth_manager = Arc::new(AuthManager::new(
            codex_home.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ));
        let provider = provider_for(server.uri());
        let manager = ModelsManager::with_provider(auth_manager, provider);

        manager
            .refresh_available_models(&config)
            .await
            .expect("initial refresh succeeds");

        // Rewrite cache with an old timestamp so it is treated as stale.
        let cache_path = codex_home.path().join(MODEL_CACHE_FILE);
        let contents =
            std::fs::read_to_string(&cache_path).expect("cache file should exist after refresh");
        let mut cache: ModelsCache =
            serde_json::from_str(&contents).expect("cache should deserialize");
        cache.fetched_at = Utc::now() - chrono::Duration::hours(1);
        std::fs::write(&cache_path, serde_json::to_string_pretty(&cache).unwrap())
            .expect("cache rewrite succeeds");

        let updated_models = vec![remote_model("fresh", "Fresh", 9)];
        server.reset().await;
        let refreshed_mock = mount_models_once(
            &server,
            ModelsResponse {
                models: updated_models.clone(),
                etag: String::new(),
            },
        )
        .await;

        manager
            .refresh_available_models(&config)
            .await
            .expect("second refresh succeeds");
        assert_eq!(
            *manager.remote_models.read().await,
            updated_models,
            "stale cache should trigger refetch"
        );
        assert_eq!(
            initial_mock.requests().len(),
            1,
            "initial refresh should only hit /models once"
        );
        assert_eq!(
            refreshed_mock.requests().len(),
            1,
            "stale cache refresh should fetch /models once"
        );
    }
}
