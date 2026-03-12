use super::cache::ModelsCacheManager;
use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::auth::AuthManager;
use crate::auth::AuthMode;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::error::CodexErr;
use crate::error::Result as CoreResult;
use crate::model_provider_info::ModelProviderInfo;
use crate::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use crate::models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;
use crate::models_manager::model_info;
use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use http::HeaderMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::TryLockError;
use tokio::time::timeout;
use tracing::error;
use tracing::info;

const MODEL_CACHE_FILE: &str = "models_cache.json";
const DEFAULT_MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
const MODELS_REFRESH_TIMEOUT: Duration = Duration::from_secs(5);

/// Strategy for refreshing available models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStrategy {
    /// Always fetch from the network, ignoring cache.
    Online,
    /// Only use cached data, never fetch from the network.
    Offline,
    /// Use cache if available and fresh, otherwise fetch from the network.
    OnlineIfUncached,
}

/// How the manager's base catalog is sourced for the lifetime of the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogMode {
    /// Start from bundled `models.json` and allow cache/network refresh updates.
    Default,
    /// Use a caller-provided catalog as authoritative and do not mutate it via refresh.
    Custom,
}

/// Coordinates remote model discovery plus cached metadata on disk.
#[derive(Debug)]
pub struct ModelsManager {
    remote_models: RwLock<Vec<ModelInfo>>,
    catalog_mode: CatalogMode,
    collaboration_modes_config: CollaborationModesConfig,
    auth_manager: Arc<AuthManager>,
    etag: RwLock<Option<String>>,
    cache_manager: ModelsCacheManager,
    provider: ModelProviderInfo,
}

impl ModelsManager {
    /// Construct a manager scoped to the provided `AuthManager`.
    ///
    /// Uses `codex_home` to store cached model metadata and initializes with bundled catalog
    /// When `model_catalog` is provided, it becomes the authoritative remote model list and
    /// background refreshes from `/models` are disabled.
    pub fn new(
        codex_home: PathBuf,
        auth_manager: Arc<AuthManager>,
        model_catalog: Option<ModelsResponse>,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Self {
        let cache_path = codex_home.join(MODEL_CACHE_FILE);
        let cache_manager = ModelsCacheManager::new(cache_path, DEFAULT_MODEL_CACHE_TTL);
        let catalog_mode = if model_catalog.is_some() {
            CatalogMode::Custom
        } else {
            CatalogMode::Default
        };
        let remote_models = model_catalog
            .map(|catalog| catalog.models)
            .unwrap_or_else(|| {
                Self::load_remote_models_from_file()
                    .unwrap_or_else(|err| panic!("failed to load bundled models.json: {err}"))
            });
        Self {
            remote_models: RwLock::new(remote_models),
            catalog_mode,
            collaboration_modes_config,
            auth_manager,
            etag: RwLock::new(None),
            cache_manager,
            provider: ModelProviderInfo::create_openai_provider(),
        }
    }

    /// List all available models, refreshing according to the specified strategy.
    ///
    /// Returns model presets sorted by priority and filtered by auth mode and visibility.
    pub async fn list_models(&self, refresh_strategy: RefreshStrategy) -> Vec<ModelPreset> {
        if let Err(err) = self.refresh_available_models(refresh_strategy).await {
            error!("failed to refresh available models: {err}");
        }
        let remote_models = self.get_remote_models().await;
        self.build_available_models(remote_models)
    }

    /// List collaboration mode presets.
    ///
    /// Returns a static set of presets seeded with the configured model.
    pub fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.list_collaboration_modes_for_config(self.collaboration_modes_config)
    }

    pub fn list_collaboration_modes_for_config(
        &self,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Vec<CollaborationModeMask> {
        builtin_collaboration_mode_presets(collaboration_modes_config)
    }

    /// Attempt to list models without blocking, using the current cached state.
    ///
    /// Returns an error if the internal lock cannot be acquired.
    pub fn try_list_models(&self) -> Result<Vec<ModelPreset>, TryLockError> {
        let remote_models = self.try_get_remote_models()?;
        Ok(self.build_available_models(remote_models))
    }

    // todo(aibrahim): should be visible to core only and sent on session_configured event
    /// Get the model identifier to use, refreshing according to the specified strategy.
    ///
    /// If `model` is provided, returns it directly. Otherwise selects the default based on
    /// auth mode and available models.
    pub async fn get_default_model(
        &self,
        model: &Option<String>,
        refresh_strategy: RefreshStrategy,
    ) -> String {
        if let Some(model) = model.as_ref() {
            return model.to_string();
        }
        if let Err(err) = self.refresh_available_models(refresh_strategy).await {
            error!("failed to refresh available models: {err}");
        }
        let remote_models = self.get_remote_models().await;
        let available = self.build_available_models(remote_models);
        available
            .iter()
            .find(|model| model.is_default)
            .or_else(|| available.first())
            .map(|model| model.model.clone())
            .unwrap_or_default()
    }

    // todo(aibrahim): look if we can tighten it to pub(crate)
    /// Look up model metadata, applying remote overrides and config adjustments.
    pub async fn get_model_info(&self, model: &str, config: &Config) -> ModelInfo {
        let remote_models = self.get_remote_models().await;
        Self::construct_model_info_from_candidates(model, &remote_models, config)
    }

    fn find_model_by_longest_prefix(model: &str, candidates: &[ModelInfo]) -> Option<ModelInfo> {
        let mut best: Option<ModelInfo> = None;
        for candidate in candidates {
            if !model.starts_with(&candidate.slug) {
                continue;
            }
            let is_better_match = if let Some(current) = best.as_ref() {
                candidate.slug.len() > current.slug.len()
            } else {
                true
            };
            if is_better_match {
                best = Some(candidate.clone());
            }
        }
        best
    }

    /// Retry metadata lookup for a single namespaced slug like `namespace/model-name`.
    ///
    /// This only strips one leading namespace segment and only when the namespace is ASCII
    /// alphanumeric/underscore (`\\w+`) to avoid broadly matching arbitrary aliases.
    fn find_model_by_namespaced_suffix(model: &str, candidates: &[ModelInfo]) -> Option<ModelInfo> {
        let (namespace, suffix) = model.split_once('/')?;
        if suffix.contains('/') {
            return None;
        }
        if !namespace
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }
        Self::find_model_by_longest_prefix(suffix, candidates)
    }

    fn construct_model_info_from_candidates(
        model: &str,
        candidates: &[ModelInfo],
        config: &Config,
    ) -> ModelInfo {
        // First use the normal longest-prefix match. If that misses, allow a narrowly scoped
        // retry for namespaced slugs like `custom/gpt-5.3-codex`.
        let remote = Self::find_model_by_longest_prefix(model, candidates)
            .or_else(|| Self::find_model_by_namespaced_suffix(model, candidates));
        let model_info = if let Some(remote) = remote {
            ModelInfo {
                slug: model.to_string(),
                used_fallback_model_metadata: false,
                ..remote
            }
        } else {
            model_info::model_info_from_slug(model)
        };
        model_info::with_config_overrides(model_info, config)
    }

    /// Refresh models if the provided ETag differs from the cached ETag.
    ///
    /// Uses `Online` strategy to fetch latest models when ETags differ.
    pub(crate) async fn refresh_if_new_etag(&self, etag: String) {
        let current_etag = self.get_etag().await;
        if current_etag.clone().is_some() && current_etag.as_deref() == Some(etag.as_str()) {
            if let Err(err) = self.cache_manager.renew_cache_ttl().await {
                error!("failed to renew cache TTL: {err}");
            }
            return;
        }
        if let Err(err) = self.refresh_available_models(RefreshStrategy::Online).await {
            error!("failed to refresh available models: {err}");
        }
    }

    /// Refresh available models according to the specified strategy.
    async fn refresh_available_models(&self, refresh_strategy: RefreshStrategy) -> CoreResult<()> {
        // don't override the custom model catalog if one was provided by the user
        if matches!(self.catalog_mode, CatalogMode::Custom) {
            return Ok(());
        }

        if self.auth_manager.auth_mode() != Some(AuthMode::Chatgpt) {
            if matches!(
                refresh_strategy,
                RefreshStrategy::Offline | RefreshStrategy::OnlineIfUncached
            ) {
                self.try_load_cache().await;
            }
            return Ok(());
        }

        match refresh_strategy {
            RefreshStrategy::Offline => {
                // Only try to load from cache, never fetch
                self.try_load_cache().await;
                Ok(())
            }
            RefreshStrategy::OnlineIfUncached => {
                // Try cache first, fall back to online if unavailable
                if self.try_load_cache().await {
                    info!("models cache: using cached models for OnlineIfUncached");
                    return Ok(());
                }
                info!("models cache: cache miss, fetching remote models");
                self.fetch_and_update_models().await
            }
            RefreshStrategy::Online => {
                // Always fetch from network
                self.fetch_and_update_models().await
            }
        }
    }

    async fn fetch_and_update_models(&self) -> CoreResult<()> {
        let _timer =
            codex_otel::start_global_timer("codex.remote_models.fetch_update.duration_ms", &[]);
        let auth = self.auth_manager.auth().await;
        let auth_mode = self.auth_manager.auth_mode();
        let api_provider = self.provider.to_api_provider(auth_mode)?;
        let api_auth = auth_provider_from_auth(auth.clone(), &self.provider)?;
        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ModelsClient::new(transport, api_provider, api_auth);

        let client_version = crate::models_manager::client_version_to_whole();
        let (models, etag) = timeout(
            MODELS_REFRESH_TIMEOUT,
            client.list_models(&client_version, HeaderMap::new()),
        )
        .await
        .map_err(|_| CodexErr::Timeout)?
        .map_err(map_api_error)?;

        self.apply_remote_models(models.clone()).await;
        *self.etag.write().await = etag.clone();
        self.cache_manager
            .persist_cache(&models, etag, client_version)
            .await;
        Ok(())
    }

    async fn get_etag(&self) -> Option<String> {
        self.etag.read().await.clone()
    }

    /// Replace the cached remote models and rebuild the derived presets list.
    async fn apply_remote_models(&self, models: Vec<ModelInfo>) {
        let mut existing_models = Self::load_remote_models_from_file().unwrap_or_default();
        for model in models {
            if let Some(existing_index) = existing_models
                .iter()
                .position(|existing| existing.slug == model.slug)
            {
                existing_models[existing_index] = model;
            } else {
                existing_models.push(model);
            }
        }
        *self.remote_models.write().await = existing_models;
    }

    fn load_remote_models_from_file() -> Result<Vec<ModelInfo>, std::io::Error> {
        let file_contents = include_str!("../../models.json");
        let response: ModelsResponse = serde_json::from_str(file_contents)?;
        Ok(response.models)
    }

    /// Attempt to satisfy the refresh from the cache when it matches the provider and TTL.
    async fn try_load_cache(&self) -> bool {
        let _timer =
            codex_otel::start_global_timer("codex.remote_models.load_cache.duration_ms", &[]);
        let client_version = crate::models_manager::client_version_to_whole();
        info!(client_version, "models cache: evaluating cache eligibility");
        let cache = match self.cache_manager.load_fresh(&client_version).await {
            Some(cache) => cache,
            None => {
                info!("models cache: no usable cache entry");
                return false;
            }
        };
        let models = cache.models.clone();
        *self.etag.write().await = cache.etag.clone();
        self.apply_remote_models(models.clone()).await;
        info!(
            models_count = models.len(),
            etag = ?cache.etag,
            "models cache: cache entry applied"
        );
        true
    }

    /// Build picker-ready presets from the active catalog snapshot.
    fn build_available_models(&self, mut remote_models: Vec<ModelInfo>) -> Vec<ModelPreset> {
        remote_models.sort_by(|a, b| a.priority.cmp(&b.priority));

        let mut presets: Vec<ModelPreset> = remote_models.into_iter().map(Into::into).collect();
        let chatgpt_mode = matches!(self.auth_manager.auth_mode(), Some(AuthMode::Chatgpt));
        presets = ModelPreset::filter_by_auth(presets, chatgpt_mode);

        ModelPreset::mark_default_by_picker_visibility(&mut presets);

        presets
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.remote_models.read().await.clone()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(self.remote_models.try_read()?.clone())
    }

    /// Construct a manager with a specific provider for testing.
    pub(crate) fn with_provider_for_tests(
        codex_home: PathBuf,
        auth_manager: Arc<AuthManager>,
        provider: ModelProviderInfo,
    ) -> Self {
        let cache_path = codex_home.join(MODEL_CACHE_FILE);
        let cache_manager = ModelsCacheManager::new(cache_path, DEFAULT_MODEL_CACHE_TTL);
        Self {
            remote_models: RwLock::new(
                Self::load_remote_models_from_file()
                    .unwrap_or_else(|err| panic!("failed to load bundled models.json: {err}")),
            ),
            catalog_mode: CatalogMode::Default,
            collaboration_modes_config: CollaborationModesConfig::default(),
            auth_manager,
            etag: RwLock::new(None),
            cache_manager,
            provider,
        }
    }

    /// Get model identifier without consulting remote state or cache.
    pub(crate) fn get_model_offline_for_tests(model: Option<&str>) -> String {
        if let Some(model) = model {
            return model.to_string();
        }
        let mut models = Self::load_remote_models_from_file().unwrap_or_default();
        models.sort_by(|a, b| a.priority.cmp(&b.priority));
        let presets: Vec<ModelPreset> = models.into_iter().map(Into::into).collect();
        presets
            .iter()
            .find(|preset| preset.show_in_picker)
            .or_else(|| presets.first())
            .map(|preset| preset.model.clone())
            .unwrap_or_default()
    }

    /// Build `ModelInfo` without consulting remote state or cache.
    pub(crate) fn construct_model_info_offline_for_tests(
        model: &str,
        config: &Config,
    ) -> ModelInfo {
        let candidates: &[ModelInfo] = if let Some(model_catalog) = config.model_catalog.as_ref() {
            &model_catalog.models
        } else {
            &[]
        };
        Self::construct_model_info_from_candidates(model, candidates, config)
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
