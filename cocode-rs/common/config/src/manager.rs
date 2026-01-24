//! Configuration manager with caching and runtime switching.
//!
//! `ConfigManager` is the main entry point for configuration management.
//! It handles loading, caching, and runtime switching of providers and models.

use crate::builtin;
use crate::error::ConfigError;
use crate::loader::ConfigLoader;
use crate::resolver::ConfigResolver;
use crate::toml_config::ConfigToml;
use crate::toml_config::LoggingConfig;
use crate::types::ActiveState;
use crate::types::ModelSummary;
use crate::types::ProviderConfig;
use crate::types::ProviderSummary;
use crate::types::ProviderType;
use crate::types::ResolvedModelInfo;
use crate::types::ResolvedProviderConfig;
use crate::types::SessionConfigJson;
use cocode_protocol::Features;
use cocode_protocol::ModelInfo;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::debug;
use tracing::info;

/// Runtime overrides for provider/model selection.
///
/// These take highest precedence in the layered resolution.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverrides {
    /// Override provider name.
    pub model_provider: Option<String>,
    /// Override model ID.
    pub model: Option<String>,
    /// Override profile name.
    pub profile: Option<String>,
}

/// Configuration manager for multi-provider setup.
///
/// Provides thread-safe configuration management with:
/// - Lazy loading from JSON and TOML files
/// - Caching with manual reload
/// - Runtime provider/model switching
/// - Profile support for quick switching
/// - Layered resolution: Runtime > TOML > Active JSON > Profile > Built-in
///
/// # Example
///
/// ```no_run
/// use cocode_config::ConfigManager;
/// use cocode_config::error::ConfigError;
///
/// # fn example() -> Result<(), ConfigError> {
/// // Load from default path (~/.cocode)
/// let manager = ConfigManager::from_default()?;
///
/// // Get current provider/model
/// let (provider, model) = manager.current();
/// println!("Current: {provider}/{model}");
///
/// // Switch to a different model
/// manager.switch("anthropic", "claude-sonnet-4-20250514")?;
///
/// // Get resolved model info
/// let info = manager.resolve_model_info("anthropic", "claude-sonnet-4-20250514")?;
/// println!("Context window: {}", info.context_window);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ConfigManager {
    /// Path to the configuration directory.
    config_path: PathBuf,
    /// Configuration loader.
    loader: ConfigLoader,
    /// Cached resolver.
    resolver: RwLock<ConfigResolver>,
    /// Current active state (from active.json).
    active: RwLock<ActiveState>,
    /// TOML configuration (from config.toml).
    config_toml: RwLock<ConfigToml>,
    /// Runtime overrides (highest precedence).
    runtime_overrides: RwLock<RuntimeOverrides>,
}

impl ConfigManager {
    /// Create a manager for the default config directory (~/.cocode).
    ///
    /// Loads configuration files if they exist, otherwise uses built-in defaults.
    pub fn from_default() -> Result<Self, ConfigError> {
        let loader = ConfigLoader::default();
        Self::from_loader(loader)
    }

    /// Create a manager for a specific config directory.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let loader = ConfigLoader::from_path(path);
        Self::from_loader(loader)
    }

    /// Create a manager from a loader.
    fn from_loader(loader: ConfigLoader) -> Result<Self, ConfigError> {
        // Ensure built-in defaults are initialized
        builtin::ensure_initialized();

        let config_path = loader.config_dir().to_path_buf();
        let loaded = loader.load_all()?;

        let resolver = ConfigResolver::with_config_dir(
            loaded.models,
            loaded.providers,
            loaded.profiles,
            &config_path,
        );

        let active = loaded.active;
        let config_toml = loaded.config_toml;

        debug!(
            path = %config_path.display(),
            "Loaded configuration"
        );

        Ok(Self {
            config_path,
            loader,
            resolver: RwLock::new(resolver),
            active: RwLock::new(active),
            config_toml: RwLock::new(config_toml),
            runtime_overrides: RwLock::new(RuntimeOverrides::default()),
        })
    }

    /// Create an empty manager with only built-in defaults.
    pub fn empty() -> Self {
        builtin::ensure_initialized();

        Self {
            config_path: PathBuf::new(),
            loader: ConfigLoader::from_path(""),
            resolver: RwLock::new(ConfigResolver::empty()),
            active: RwLock::new(ActiveState::default()),
            config_toml: RwLock::new(ConfigToml::default()),
            runtime_overrides: RwLock::new(RuntimeOverrides::default()),
        }
    }

    /// Get the configuration directory path.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Resolve model info with all layers merged.
    pub fn resolve_model_info(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<ResolvedModelInfo, ConfigError> {
        let resolver = self
            .resolver
            .read()
            .map_err(|e| ConfigError::io(format!("Failed to acquire read lock: {e}")))?;
        resolver.resolve_model_info(provider, model)
    }

    /// Resolve provider configuration.
    pub fn resolve_provider(&self, provider: &str) -> Result<ResolvedProviderConfig, ConfigError> {
        let resolver = self
            .resolver
            .read()
            .map_err(|e| ConfigError::io(format!("Failed to acquire read lock: {e}")))?;
        resolver.resolve_provider(provider)
    }

    /// Get the current active provider and model.
    ///
    /// Resolution order (highest to lowest precedence):
    /// 1. Runtime overrides (set via `set_runtime_overrides()`)
    /// 2. TOML config (`config.toml`)
    /// 3. Active state (`active.json`)
    /// 4. Default profile
    /// 5. Built-in defaults ("openai", "gpt-5")
    pub fn current(&self) -> (String, String) {
        // 1. Check runtime overrides first
        let runtime = self.runtime_overrides.read().unwrap();
        if let (Some(provider), Some(model)) = (&runtime.model_provider, &runtime.model) {
            return (provider.clone(), model.clone());
        }
        drop(runtime);

        // 2. Check TOML config
        let toml = self.config_toml.read().unwrap();
        if let (Some(provider), Some(model)) = (&toml.model_provider, &toml.model) {
            return (provider.clone(), model.clone());
        }
        drop(toml);

        // 3. Check active state (from active.json)
        let active = self.active.read().unwrap();
        if let (Some(provider), Some(model)) = (&active.provider, &active.model) {
            return (provider.clone(), model.clone());
        }
        drop(active);

        // 4. Try default profile
        let resolver = self.resolver.read().unwrap();
        if let Some(profile_name) = resolver.default_profile() {
            if let Ok(profile) = resolver.resolve_profile(profile_name) {
                return (profile.provider.clone(), profile.model.clone());
            }
        }

        // 5. Fallback to built-in default
        ("openai".to_string(), "gpt-5".to_string())
    }

    /// Set runtime overrides for provider/model selection.
    ///
    /// Runtime overrides take the highest precedence in layered resolution.
    pub fn set_runtime_overrides(&self, overrides: RuntimeOverrides) {
        let mut runtime = self.runtime_overrides.write().unwrap();
        *runtime = overrides;
    }

    /// Get the current runtime overrides.
    pub fn runtime_overrides(&self) -> RuntimeOverrides {
        self.runtime_overrides.read().unwrap().clone()
    }

    /// Get the TOML configuration.
    pub fn config_toml(&self) -> ConfigToml {
        self.config_toml.read().unwrap().clone()
    }

    /// Get the logging configuration from TOML.
    ///
    /// Returns `None` if no logging section is configured.
    pub fn logging_config(&self) -> Option<LoggingConfig> {
        self.config_toml.read().unwrap().logging.clone()
    }

    /// Get the current features configuration.
    ///
    /// Combines default features with TOML overrides.
    pub fn features(&self) -> Features {
        let toml = self.config_toml.read().unwrap();
        if let Some(features_toml) = &toml.features {
            features_toml.clone().into_features()
        } else {
            Features::with_defaults()
        }
    }

    /// Check if a specific feature is enabled.
    ///
    /// Uses the layered features configuration.
    pub fn is_feature_enabled(&self, feature: cocode_protocol::Feature) -> bool {
        self.features().enabled(feature)
    }

    /// Get the model max output tokens from TOML config.
    pub fn model_max_output_tokens(&self) -> Option<i32> {
        self.config_toml.read().unwrap().model_max_output_tokens
    }

    /// Switch to a specific provider and model.
    ///
    /// This updates the runtime state and persists it to `active.json`.
    pub fn switch(&self, provider: &str, model: &str) -> Result<(), ConfigError> {
        // Validate the provider/model combination
        let resolver = self
            .resolver
            .read()
            .map_err(|e| ConfigError::io(format!("Failed to acquire read lock: {e}")))?;

        // Check if provider is configured (either in config or built-in)
        if !resolver.has_provider(provider) {
            // Check built-in providers
            if builtin::get_provider_defaults(provider).is_none() {
                return Err(ConfigError::provider_not_found(provider));
            }
        }

        drop(resolver);

        // Update active state
        {
            let mut active = self
                .active
                .write()
                .map_err(|e| ConfigError::io(format!("Failed to acquire write lock: {e}")))?;

            active.provider = Some(provider.to_string());
            active.model = Some(model.to_string());
            active.profile = None; // Clear profile when switching directly

            // Persist to disk
            self.loader.save_active(&active)?;
        }

        info!(provider = provider, model = model, "Switched to new model");
        Ok(())
    }

    /// Switch to a named profile.
    ///
    /// This updates the runtime state and persists it to `active.json`.
    pub fn switch_profile(&self, profile: &str) -> Result<(), ConfigError> {
        let resolver = self
            .resolver
            .read()
            .map_err(|e| ConfigError::io(format!("Failed to acquire read lock: {e}")))?;

        let profile_config = resolver.resolve_profile(profile)?;
        let provider = profile_config.provider.clone();
        let model = profile_config.model.clone();
        let session_config = profile_config.session_config.clone();

        drop(resolver);

        // Update active state
        {
            let mut active = self
                .active
                .write()
                .map_err(|e| ConfigError::io(format!("Failed to acquire write lock: {e}")))?;

            active.provider = Some(provider.clone());
            active.model = Some(model.clone());
            active.profile = Some(profile.to_string());
            active.session_overrides = session_config;

            // Persist to disk
            self.loader.save_active(&active)?;
        }

        info!(
            profile = profile,
            provider = provider,
            model = model,
            "Switched to profile"
        );
        Ok(())
    }

    /// Reload configuration from disk.
    ///
    /// This reloads all configuration files (JSON and TOML) and updates the cached state.
    /// Note: Runtime overrides are preserved across reloads.
    pub fn reload(&self) -> Result<(), ConfigError> {
        let loaded = self.loader.load_all()?;

        let new_resolver = ConfigResolver::new(loaded.models, loaded.providers, loaded.profiles);

        {
            let mut resolver = self
                .resolver
                .write()
                .map_err(|e| ConfigError::io(format!("Failed to acquire write lock: {e}")))?;
            *resolver = new_resolver;
        }

        {
            let mut active = self
                .active
                .write()
                .map_err(|e| ConfigError::io(format!("Failed to acquire write lock: {e}")))?;
            *active = loaded.active;
        }

        {
            let mut config_toml = self
                .config_toml
                .write()
                .map_err(|e| ConfigError::io(format!("Failed to acquire write lock: {e}")))?;
            *config_toml = loaded.config_toml;
        }

        info!("Reloaded configuration");
        Ok(())
    }

    /// List all available providers.
    ///
    /// Returns providers from both configuration files and built-in defaults.
    pub fn list_providers(&self) -> Vec<ProviderSummary> {
        let resolver = self.resolver.read().unwrap();
        let mut summaries = Vec::new();

        // Add configured providers
        for name in resolver.list_providers() {
            if let Some(config) = resolver.get_provider_config(name) {
                summaries.push(ProviderSummary {
                    name: name.to_string(),
                    display_name: config.name.clone(),
                    provider_type: config.provider_type,
                    has_api_key: config.api_key.is_some() || config.env_key.is_some(),
                    model_count: config.models.len() as i32,
                });
            }
        }

        // Add built-in providers not already in config
        for name in builtin::list_builtin_providers() {
            if !summaries.iter().any(|s| s.name == name) {
                if let Some(config) = builtin::get_provider_defaults(name) {
                    summaries.push(ProviderSummary {
                        name: name.to_string(),
                        display_name: config.name,
                        provider_type: config.provider_type,
                        has_api_key: config.env_key.is_some(),
                        model_count: 0,
                    });
                }
            }
        }

        summaries
    }

    /// List models for a specific provider.
    ///
    /// Returns models from both configuration files and built-in defaults.
    pub fn list_models(&self, provider: &str) -> Vec<ModelSummary> {
        let resolver = self.resolver.read().unwrap();
        let mut summaries = Vec::new();

        // Add configured models for this provider
        for model_id in resolver.list_models(provider) {
            if let Ok(info) = resolver.resolve_model_info(provider, model_id) {
                summaries.push(ModelSummary {
                    id: model_id.to_string(),
                    display_name: info.display_name,
                    context_window: Some(info.context_window),
                    capabilities: info.capabilities,
                });
            }
        }

        // If no models configured, suggest some built-in ones based on provider type
        if summaries.is_empty() {
            if let Some(provider_config) = resolver.get_provider_config(provider) {
                let suggested = suggest_models_for_provider(provider_config.provider_type);
                for model_id in suggested {
                    if let Some(config) = builtin::get_model_defaults(model_id) {
                        summaries.push(ModelSummary {
                            id: model_id.to_string(),
                            display_name: config
                                .display_name
                                .unwrap_or_else(|| model_id.to_string()),
                            context_window: config.context_window,
                            capabilities: config.capabilities.unwrap_or_default(),
                        });
                    }
                }
            }
        }

        summaries
    }

    /// List all configured profiles.
    pub fn list_profiles(&self) -> Vec<String> {
        let resolver = self.resolver.read().unwrap();
        resolver
            .list_profiles()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Get the default profile name.
    pub fn default_profile(&self) -> Option<String> {
        let resolver = self.resolver.read().unwrap();
        resolver.default_profile().map(String::from)
    }

    /// Get session config for current active state.
    pub fn current_session_config(&self) -> Option<SessionConfigJson> {
        let active = self.active.read().unwrap();
        active.session_overrides.clone()
    }

    /// Check if a provider is available (configured or built-in).
    pub fn has_provider(&self, name: &str) -> bool {
        let resolver = self.resolver.read().unwrap();
        resolver.has_provider(name) || builtin::get_provider_defaults(name).is_some()
    }

    /// Get provider config by name.
    pub fn get_provider_config(&self, name: &str) -> Option<ProviderConfig> {
        let resolver = self.resolver.read().unwrap();
        resolver
            .get_provider_config(name)
            .cloned()
            .or_else(|| builtin::get_provider_defaults(name))
    }

    /// Get model config by ID.
    pub fn get_model_config(&self, id: &str) -> Option<ModelInfo> {
        let resolver = self.resolver.read().unwrap();
        resolver
            .get_model_config(id)
            .cloned()
            .or_else(|| builtin::get_model_defaults(id))
    }
}

/// Suggest default models based on provider type.
fn suggest_models_for_provider(provider_type: ProviderType) -> Vec<&'static str> {
    match provider_type {
        ProviderType::Openai => vec!["gpt-5", "gpt-5.2"],
        ProviderType::Anthropic => vec!["claude-sonnet-4", "claude-opus-4"],
        ProviderType::Gemini => vec!["gemini-3-pro", "gemini-3-flash"],
        ProviderType::Volcengine => vec!["deepseek-r1", "deepseek-chat"],
        ProviderType::Zai => vec!["glm-4-plus", "glm-4-flash"],
        ProviderType::OpenaiCompat => vec!["deepseek-chat", "qwen-plus"],
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self {
            config_path: self.config_path.clone(),
            loader: ConfigLoader::from_path(&self.config_path),
            resolver: RwLock::new(self.resolver.read().unwrap().clone()),
            active: RwLock::new(self.active.read().unwrap().clone()),
            config_toml: RwLock::new(self.config_toml.read().unwrap().clone()),
            runtime_overrides: RwLock::new(self.runtime_overrides.read().unwrap().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (TempDir, ConfigManager) {
        let temp_dir = TempDir::new().unwrap();

        // Create test config files
        let providers_json = r#"{
            "version": "1.0",
            "providers": {
                "test-openai": {
                    "name": "Test OpenAI",
                    "type": "openai",
                    "api_key": "test-key",
                    "default_model": "gpt-5",
                    "models": {
                        "gpt-5": {},
                        "gpt-5-mini": {}
                    }
                }
            }
        }"#;
        std::fs::write(temp_dir.path().join("providers.json"), providers_json).unwrap();

        let profiles_json = r#"{
            "version": "1.0",
            "default_profile": "default",
            "profiles": {
                "default": {
                    "provider": "test-openai",
                    "model": "gpt-5"
                },
                "fast": {
                    "provider": "test-openai",
                    "model": "gpt-5-mini",
                    "session_config": {
                        "temperature": 0.5
                    }
                }
            }
        }"#;
        std::fs::write(temp_dir.path().join("profiles.json"), profiles_json).unwrap();

        let manager = ConfigManager::from_path(temp_dir.path()).unwrap();
        (temp_dir, manager)
    }

    #[test]
    fn test_from_default_succeeds() {
        // Should succeed even if ~/.cocode doesn't exist
        let manager = ConfigManager::from_default();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_empty_manager() {
        let manager = ConfigManager::empty();
        let (provider, model) = manager.current();
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-5");
    }

    #[test]
    fn test_current_from_default_profile() {
        let (_temp, manager) = create_test_manager();
        let (provider, model) = manager.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5");
    }

    #[test]
    fn test_switch_provider_model() {
        let (_temp, manager) = create_test_manager();

        manager.switch("test-openai", "gpt-5-mini").unwrap();
        let (provider, model) = manager.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5-mini");
    }

    #[test]
    fn test_switch_profile() {
        let (_temp, manager) = create_test_manager();

        manager.switch_profile("fast").unwrap();
        let (provider, model) = manager.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5-mini");

        // Should also have session config
        let session = manager.current_session_config();
        assert!(session.is_some());
        assert_eq!(session.unwrap().temperature, Some(0.5));
    }

    #[test]
    fn test_switch_nonexistent_profile() {
        let (_temp, manager) = create_test_manager();
        let result = manager.switch_profile("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_model_info() {
        let (_temp, manager) = create_test_manager();

        let info = manager.resolve_model_info("test-openai", "gpt-5").unwrap();
        assert_eq!(info.id, "gpt-5");
        assert_eq!(info.display_name, "GPT-5");
        assert_eq!(info.context_window, 272000);
    }

    #[test]
    fn test_list_providers() {
        let (_temp, manager) = create_test_manager();

        let providers = manager.list_providers();
        assert!(providers.iter().any(|p| p.name == "test-openai"));

        // Should also include built-in providers
        assert!(providers.iter().any(|p| p.name == "openai"));
    }

    #[test]
    fn test_list_models() {
        let (_temp, manager) = create_test_manager();

        let models = manager.list_models("test-openai");
        assert!(!models.is_empty());
    }

    #[test]
    fn test_list_profiles() {
        let (_temp, manager) = create_test_manager();

        let profiles = manager.list_profiles();
        assert!(profiles.contains(&"default".to_string()));
        assert!(profiles.contains(&"fast".to_string()));
    }

    #[test]
    fn test_reload() {
        let (temp_dir, manager) = create_test_manager();

        // Modify config
        let new_profiles = r#"{
            "version": "1.0",
            "default_profile": "new-default",
            "profiles": {
                "new-default": {
                    "provider": "test-openai",
                    "model": "gpt-5-mini"
                }
            }
        }"#;
        std::fs::write(temp_dir.path().join("profiles.json"), new_profiles).unwrap();

        manager.reload().unwrap();

        assert_eq!(manager.default_profile(), Some("new-default".to_string()));
    }

    #[test]
    fn test_has_provider() {
        let (_temp, manager) = create_test_manager();

        assert!(manager.has_provider("test-openai"));
        assert!(manager.has_provider("openai")); // Built-in
        assert!(!manager.has_provider("nonexistent"));
    }

    #[test]
    fn test_get_model_config() {
        let (_temp, manager) = create_test_manager();

        // Built-in model
        let config = manager.get_model_config("gpt-5");
        assert!(config.is_some());
        assert_eq!(config.unwrap().display_name, Some("GPT-5".to_string()));
    }

    #[test]
    fn test_persist_active_state() {
        let (temp_dir, manager) = create_test_manager();

        manager.switch("test-openai", "gpt-5-mini").unwrap();

        // Create new manager and verify state persisted
        let manager2 = ConfigManager::from_path(temp_dir.path()).unwrap();
        let (provider, model) = manager2.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5-mini");
    }
}
