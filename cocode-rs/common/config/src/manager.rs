//! Configuration manager with caching and runtime switching.
//!
//! `ConfigManager` is the main entry point for configuration management.
//! It handles loading, caching, and runtime switching of providers and models.

use crate::builtin;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::env_loader::EnvLoader;
use crate::error::ConfigError;
use crate::error::NotFoundKind;
use crate::error::config_error::{InternalSnafu, NotFoundSnafu};
use crate::json_config::AppConfig;
use crate::json_config::LoggingConfig;
use crate::loader::ConfigLoader;
use crate::loader::load_instructions;
use crate::resolver::ConfigResolver;
use crate::types::ModelSummary;
use crate::types::ProviderConfig;
use crate::types::ProviderSummary;
use crate::types::ProviderType;
use crate::types::ResolvedModelInfo;
use cocode_protocol::Features;
use cocode_protocol::ModelInfo;
use cocode_protocol::ProviderInfo;
use cocode_protocol::SandboxMode;
use cocode_protocol::model::ModelRole;
use cocode_protocol::model::ModelSpec;
use std::collections::HashMap;
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
    /// Override main model (format: "provider/model").
    pub main: Option<ModelSpec>,
}

/// Configuration manager for multi-provider setup.
///
/// Provides thread-safe configuration management with:
/// - Lazy loading from JSON and TOML files
/// - Caching with manual reload
/// - Runtime provider/model switching
/// - Layered resolution: Runtime > TOML > Built-in
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
    /// Application configuration (from config.json).
    config: RwLock<AppConfig>,
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

        let resolver =
            ConfigResolver::with_config_dir(loaded.models, loaded.providers, &config_path);

        let config = loaded.config;

        debug!(
            path = %config_path.display(),
            "Loaded configuration"
        );

        Ok(Self {
            config_path,
            loader,
            resolver: RwLock::new(resolver),
            config: RwLock::new(config),
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
            config: RwLock::new(AppConfig::default()),
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
        let resolver = self.resolver.read().map_err(|e| {
            InternalSnafu {
                message: format!("Failed to acquire read lock: {e}"),
            }
            .build()
        })?;
        resolver.resolve_model_info(provider, model)
    }

    /// Resolve provider configuration into a complete `ProviderInfo`.
    ///
    /// The returned `ProviderInfo` contains:
    /// - Resolved API key (from env or config)
    /// - All connection settings (base_url, streaming, wire_api)
    /// - Map of resolved models (slug -> ModelInfo)
    pub fn resolve_provider(&self, provider: &str) -> Result<ProviderInfo, ConfigError> {
        let resolver = self.resolver.read().map_err(|e| {
            InternalSnafu {
                message: format!("Failed to acquire read lock: {e}"),
            }
            .build()
        })?;
        resolver.resolve_provider(provider)
    }

    /// Get the current active provider and model.
    ///
    /// Resolution order (highest to lowest precedence):
    /// 1. Runtime overrides (set via `set_runtime_overrides()`)
    /// 2. JSON config with profile resolution (`config.json`)
    /// 3. Built-in defaults ("openai", "gpt-5")
    pub fn current(&self) -> (String, String) {
        self.current_for_role(ModelRole::Main)
    }

    /// Get the current active provider and model for a specific role.
    ///
    /// Resolution order (highest to lowest precedence):
    /// 1. Runtime overrides (for Main role only)
    /// 2. JSON config with profile resolution (`config.json`)
    /// 3. Built-in defaults ("openai", "gpt-5")
    pub fn current_for_role(&self, role: ModelRole) -> (String, String) {
        // 1. Check runtime overrides first (only for Main role)
        if role == ModelRole::Main {
            let runtime = self.runtime_overrides.read().unwrap();
            if let Some(spec) = &runtime.main {
                return (spec.provider.clone(), spec.model.clone());
            }
        }

        // 2. Check JSON config (with profile resolution)
        let config = self.config.read().unwrap();
        let resolved = config.resolve();
        if let Some(spec) = resolved.models.get(role) {
            return (spec.provider.clone(), spec.model.clone());
        }
        drop(config);

        // 3. Fallback to built-in default
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

    /// Get the application configuration.
    pub fn app_config(&self) -> AppConfig {
        self.config.read().unwrap().clone()
    }

    /// Get the logging configuration from config.json.
    ///
    /// Returns `None` if no logging section is configured.
    pub fn logging_config(&self) -> Option<LoggingConfig> {
        self.config.read().unwrap().logging.clone()
    }

    /// Get the current features configuration.
    ///
    /// Combines default features with config overrides and profile overrides.
    pub fn features(&self) -> Features {
        let config = self.config.read().unwrap();
        config.resolve().features
    }

    /// Check if a specific feature is enabled.
    ///
    /// Uses the layered features configuration.
    pub fn is_feature_enabled(&self, feature: cocode_protocol::Feature) -> bool {
        self.features().enabled(feature)
    }

    /// Switch to a specific provider and model.
    ///
    /// This updates the runtime overrides (in-memory only).
    /// To persist, edit `config.toml` directly.
    pub fn switch(&self, provider: &str, model: &str) -> Result<(), ConfigError> {
        // Validate the provider/model combination
        let resolver = self.resolver.read().map_err(|e| {
            InternalSnafu {
                message: format!("Failed to acquire read lock: {e}"),
            }
            .build()
        })?;

        // Check if provider is configured (either in config or built-in)
        if !resolver.has_provider(provider) {
            // Check built-in providers
            if builtin::get_provider_defaults(provider).is_none() {
                return NotFoundSnafu {
                    kind: NotFoundKind::Provider,
                    name: provider.to_string(),
                }
                .fail();
            }
        }

        drop(resolver);

        // Update runtime overrides (in-memory)
        {
            let mut runtime = self.runtime_overrides.write().map_err(|e| {
                InternalSnafu {
                    message: format!("Failed to acquire write lock: {e}"),
                }
                .build()
            })?;

            runtime.main = Some(ModelSpec::new(provider, model));
        }

        info!(provider = provider, model = model, "Switched to new model");
        Ok(())
    }

    /// Reload configuration from disk.
    ///
    /// This reloads all configuration files (JSON) and updates the cached state.
    /// Note: Runtime overrides are preserved across reloads.
    ///
    /// For empty managers (created via `empty()`), this is a no-op.
    pub fn reload(&self) -> Result<(), ConfigError> {
        // Empty managers have no config files to reload
        if self.config_path.as_os_str().is_empty() {
            debug!("Skipping reload for empty manager (no config path)");
            return Ok(());
        }

        let loaded = self.loader.load_all()?;

        let new_resolver =
            ConfigResolver::with_config_dir(loaded.models, loaded.providers, &self.config_path);

        {
            let mut resolver = self.resolver.write().map_err(|e| {
                InternalSnafu {
                    message: format!("Failed to acquire write lock: {e}"),
                }
                .build()
            })?;
            *resolver = new_resolver;
        }

        {
            let mut config = self.config.write().map_err(|e| {
                InternalSnafu {
                    message: format!("Failed to acquire write lock: {e}"),
                }
                .build()
            })?;
            *config = loaded.config;
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

    /// Build a complete Config snapshot from current state.
    ///
    /// This method creates a complete runtime configuration snapshot that includes:
    /// - All resolved model roles
    /// - All available providers with resolved API keys
    /// - Features from config with defaults applied
    /// - Paths (cwd, cocode_home)
    /// - User instructions from AGENTS.md
    /// - Sandbox configuration
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cocode_config::{ConfigManager, ConfigOverrides};
    /// use cocode_protocol::model::ModelRole;
    ///
    /// # fn example() -> Result<(), cocode_config::error::ConfigError> {
    /// let manager = ConfigManager::from_default()?;
    /// let config = manager.build_config(ConfigOverrides::default())?;
    ///
    /// // Access main model
    /// if let Some(main) = config.main_model_info() {
    ///     println!("Main: {} ({})", main.display_name, main.context_window);
    /// }
    ///
    /// // Access fast model (falls back to main if not configured)
    /// if let Some(fast) = config.model_for_role(ModelRole::Fast) {
    ///     println!("Fast: {}", fast.display_name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_config(&self, overrides: ConfigOverrides) -> Result<Config, ConfigError> {
        // Get resolved app config (with profile applied)
        let app_config = self.config.read().unwrap();
        let resolved = app_config.resolve();

        // Merge model overrides
        let mut models = resolved.models.clone();
        if let Some(override_models) = &overrides.models {
            models.merge(override_models);
        }

        // Resolve all configured roles -> ResolvedModelInfo
        let mut resolved_models = HashMap::new();
        for role in ModelRole::all() {
            if let Some(spec) = models.get(*role) {
                if let Ok(info) = self.resolve_model_info(&spec.provider, &spec.model) {
                    resolved_models.insert(*role, info);
                }
            }
        }

        // Resolve all providers
        let mut providers = HashMap::new();
        for summary in self.list_providers() {
            if let Ok(info) = self.resolve_provider(&summary.name) {
                providers.insert(summary.name.clone(), info);
            }
        }

        // Determine cwd
        let cwd = overrides
            .cwd
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        // Load instructions from cwd
        let user_instructions = overrides
            .user_instructions
            .or_else(|| load_instructions(&cwd));

        // Apply feature overrides
        let mut features = resolved.features.clone();
        if !overrides.features.is_empty() {
            features.apply_map(
                &overrides
                    .features
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect(),
            );
        }

        // Build writable roots (default to cwd if WorkspaceWrite)
        let sandbox_mode = overrides.sandbox_mode.unwrap_or_default();
        let writable_roots = overrides.writable_roots.unwrap_or_else(|| {
            if sandbox_mode == SandboxMode::WorkspaceWrite {
                vec![cwd.clone()]
            } else {
                Vec::new()
            }
        });

        // Load extended configs from environment variables
        // Precedence: overrides > env vars > JSON config > defaults
        let env_loader = EnvLoader::new();

        // Tool config: overrides > env > JSON > default
        let tool_config = overrides.tool_config.unwrap_or_else(|| {
            let mut config = env_loader.load_tool_config();
            if let Some(json_config) = &resolved.tool {
                // JSON config fills in gaps where env didn't set values
                // For numeric fields: use JSON if env produced the default value
                if config.max_tool_concurrency == cocode_protocol::DEFAULT_MAX_TOOL_CONCURRENCY {
                    config.max_tool_concurrency = json_config.max_tool_concurrency;
                }
                if config.mcp_tool_timeout.is_none() {
                    config.mcp_tool_timeout = json_config.mcp_tool_timeout;
                }
            }
            config
        });

        // Compact config: overrides > env > JSON > default
        let compact_config = overrides.compact_config.unwrap_or_else(|| {
            let mut config = env_loader.load_compact_config();
            if let Some(json_config) = &resolved.compact {
                // Merge ALL JSON values where env didn't set them
                // Boolean fields: OR logic (true from either source wins)
                if !config.disable_compact && json_config.disable_compact {
                    config.disable_compact = true;
                }
                if !config.disable_auto_compact && json_config.disable_auto_compact {
                    config.disable_auto_compact = true;
                }
                if !config.disable_micro_compact && json_config.disable_micro_compact {
                    config.disable_micro_compact = true;
                }
                // Option fields: use JSON if env didn't set
                if config.autocompact_pct_override.is_none() {
                    config.autocompact_pct_override = json_config.autocompact_pct_override;
                }
                if config.blocking_limit_override.is_none() {
                    config.blocking_limit_override = json_config.blocking_limit_override;
                }
                // Numeric fields: use JSON if env produced the default value
                if config.session_memory_min_tokens
                    == cocode_protocol::DEFAULT_SESSION_MEMORY_MIN_TOKENS
                {
                    config.session_memory_min_tokens = json_config.session_memory_min_tokens;
                }
                if config.session_memory_max_tokens
                    == cocode_protocol::DEFAULT_SESSION_MEMORY_MAX_TOKENS
                {
                    config.session_memory_max_tokens = json_config.session_memory_max_tokens;
                }
                if config.extraction_cooldown_secs
                    == cocode_protocol::DEFAULT_EXTRACTION_COOLDOWN_SECS
                {
                    config.extraction_cooldown_secs = json_config.extraction_cooldown_secs;
                }
                if config.context_restore_max_files
                    == cocode_protocol::DEFAULT_CONTEXT_RESTORE_MAX_FILES
                {
                    config.context_restore_max_files = json_config.context_restore_max_files;
                }
                if config.context_restore_budget == cocode_protocol::DEFAULT_CONTEXT_RESTORE_BUDGET
                {
                    config.context_restore_budget = json_config.context_restore_budget;
                }
            }
            // Validate and log warning if invalid
            if let Err(e) = config.validate() {
                tracing::warn!(error = %e, "Invalid compact config");
            }
            config
        });

        // Plan config: overrides > env > JSON > default
        let plan_config = overrides.plan_config.unwrap_or_else(|| {
            let mut config = env_loader.load_plan_config();
            if let Some(json_config) = &resolved.plan {
                // Merge JSON values where env didn't set them
                // Note: env loader already calls clamp_all(), so we check against clamped defaults
                if config.agent_count == cocode_protocol::DEFAULT_PLAN_AGENT_COUNT {
                    config.agent_count = json_config.agent_count.clamp(
                        cocode_protocol::MIN_AGENT_COUNT,
                        cocode_protocol::MAX_AGENT_COUNT,
                    );
                }
                if config.explore_agent_count == cocode_protocol::DEFAULT_PLAN_EXPLORE_AGENT_COUNT {
                    config.explore_agent_count = json_config.explore_agent_count.clamp(
                        cocode_protocol::MIN_AGENT_COUNT,
                        cocode_protocol::MAX_AGENT_COUNT,
                    );
                }
            }
            // Validate and log warning if invalid (should always pass after clamp)
            if let Err(e) = config.validate() {
                tracing::warn!(error = %e, "Invalid plan config");
            }
            config
        });

        // Attachment config: overrides > env > JSON > default
        let attachment_config = overrides.attachment_config.unwrap_or_else(|| {
            let mut config = env_loader.load_attachment_config();
            if let Some(json_config) = &resolved.attachment {
                // Merge JSON values where env didn't set them
                // Boolean fields: OR logic (true from either source wins)
                if !config.disable_attachments && json_config.disable_attachments {
                    config.disable_attachments = true;
                }
                if !config.enable_token_usage_attachment
                    && json_config.enable_token_usage_attachment
                {
                    config.enable_token_usage_attachment = true;
                }
            }
            config
        });

        // Path config: overrides > env > JSON > default
        let mut path_config = overrides.path_config.unwrap_or_else(|| {
            let config = env_loader.load_path_config();
            config
        });
        // Merge JSON path config
        if let Some(json_paths) = &resolved.paths {
            if path_config.project_dir.is_none() {
                path_config.project_dir = json_paths.project_dir.clone();
            }
            if path_config.plugin_root.is_none() {
                path_config.plugin_root = json_paths.plugin_root.clone();
            }
            if path_config.env_file.is_none() {
                path_config.env_file = json_paths.env_file.clone();
            }
        }

        Ok(Config {
            models,
            providers,
            resolved_models,
            cwd,
            cocode_home: self.config_path.clone(),
            user_instructions,
            features,
            logging: resolved.logging,
            active_profile: app_config.profile.clone(),
            ephemeral: overrides.ephemeral.unwrap_or(false),
            sandbox_mode,
            writable_roots,
            tool_config,
            compact_config,
            plan_config,
            attachment_config,
            path_config,
        })
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
            config: RwLock::new(self.config.read().unwrap().clone()),
            runtime_overrides: RwLock::new(self.runtime_overrides.read().unwrap().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::CONFIG_FILE;
    use tempfile::TempDir;

    fn create_test_manager() -> (TempDir, ConfigManager) {
        let temp_dir = TempDir::new().unwrap();

        // Create test config files (new list format for *provider.json)
        let providers_json = r#"[
            {
                "name": "test-openai",
                "type": "openai",
                "base_url": "https://api.openai.com/v1",
                "api_key": "test-key",
                "models": [
                    {"slug": "gpt-5"},
                    {"slug": "gpt-5-mini"}
                ]
            }
        ]"#;
        std::fs::write(temp_dir.path().join("provider.json"), providers_json).unwrap();

        // Create config.json
        let config_json = r#"{
            "models": {
                "main": "test-openai/gpt-5"
            }
        }"#;
        std::fs::write(temp_dir.path().join(CONFIG_FILE), config_json).unwrap();

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
    fn test_current_from_config() {
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
    fn test_reload() {
        let (temp_dir, manager) = create_test_manager();

        // Modify config
        let new_config_json = r#"{
            "models": {
                "main": "test-openai/gpt-5-mini"
            }
        }"#;
        std::fs::write(temp_dir.path().join(CONFIG_FILE), new_config_json).unwrap();

        manager.reload().unwrap();

        // Reset runtime overrides to use JSON config
        manager.set_runtime_overrides(RuntimeOverrides::default());

        let (provider, model) = manager.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5-mini");
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
    fn test_runtime_switch_is_in_memory() {
        let (temp_dir, manager) = create_test_manager();

        manager.switch("test-openai", "gpt-5-mini").unwrap();
        let (provider, model) = manager.current();
        assert_eq!(provider, "test-openai");
        assert_eq!(model, "gpt-5-mini");

        // Create new manager - switch should NOT persist (in-memory only)
        let manager2 = ConfigManager::from_path(temp_dir.path()).unwrap();
        let (provider2, model2) = manager2.current();
        // Should fall back to JSON config
        assert_eq!(provider2, "test-openai");
        assert_eq!(model2, "gpt-5"); // Default from config.json, not gpt-5-mini
    }

    // ==========================================================
    // Tests for build_config
    // ==========================================================

    #[test]
    fn test_build_config_basic() {
        let (_temp, manager) = create_test_manager();

        let config = manager.build_config(ConfigOverrides::default()).unwrap();

        // Should have main model resolved
        assert!(config.main_model_info().is_some());
        let main = config.main_model_info().unwrap();
        assert_eq!(main.id, "gpt-5");
        assert_eq!(main.display_name, "GPT-5");

        // Should have providers resolved
        assert!(config.providers.contains_key("test-openai"));

        // Should have default sandbox mode
        assert_eq!(config.sandbox_mode, SandboxMode::default());
        assert!(!config.ephemeral);
    }

    #[test]
    fn test_build_config_with_overrides() {
        let (_temp, manager) = create_test_manager();

        let overrides = ConfigOverrides::new()
            .with_cwd("/custom/path")
            .with_sandbox_mode(SandboxMode::WorkspaceWrite)
            .with_ephemeral(true);

        let config = manager.build_config(overrides).unwrap();

        assert_eq!(config.cwd, PathBuf::from("/custom/path"));
        assert_eq!(config.sandbox_mode, SandboxMode::WorkspaceWrite);
        assert!(config.ephemeral);

        // Default writable root should be cwd for WorkspaceWrite
        assert!(
            config
                .writable_roots
                .contains(&PathBuf::from("/custom/path"))
        );
    }

    #[test]
    fn test_build_config_with_custom_writable_roots() {
        let (_temp, manager) = create_test_manager();

        let overrides = ConfigOverrides::new()
            .with_sandbox_mode(SandboxMode::WorkspaceWrite)
            .with_writable_roots(vec![PathBuf::from("/a"), PathBuf::from("/b")]);

        let config = manager.build_config(overrides).unwrap();

        assert_eq!(config.writable_roots.len(), 2);
        assert!(config.writable_roots.contains(&PathBuf::from("/a")));
        assert!(config.writable_roots.contains(&PathBuf::from("/b")));
    }

    #[test]
    fn test_build_config_role_fallback() {
        let (_temp, manager) = create_test_manager();

        let config = manager.build_config(ConfigOverrides::default()).unwrap();

        // Fast role should fall back to main
        let fast = config.model_for_role(ModelRole::Fast);
        assert!(fast.is_some());
        assert_eq!(fast.unwrap().id, "gpt-5"); // Falls back to main

        // Vision role should also fall back to main
        let vision = config.model_for_role(ModelRole::Vision);
        assert!(vision.is_some());
        assert_eq!(vision.unwrap().id, "gpt-5");
    }

    #[test]
    fn test_build_config_empty_manager() {
        let manager = ConfigManager::empty();
        let config = manager.build_config(ConfigOverrides::default()).unwrap();

        // Empty manager has no main model configured, so resolved_models is empty
        assert!(config.main_model_info().is_none());
        assert!(config.models.is_empty());
    }

    #[test]
    fn test_build_config_with_user_instructions() {
        let (_temp, manager) = create_test_manager();

        let overrides =
            ConfigOverrides::new().with_user_instructions("Custom instructions for testing");

        let config = manager.build_config(overrides).unwrap();

        assert_eq!(
            config.user_instructions,
            Some("Custom instructions for testing".to_string())
        );
    }

    #[test]
    fn test_build_config_feature_overrides() {
        let (_temp, manager) = create_test_manager();

        let overrides = ConfigOverrides::new().with_feature("subagent", true);

        let config = manager.build_config(overrides).unwrap();

        assert!(config.is_feature_enabled(cocode_protocol::Feature::Subagent));
    }

    #[test]
    fn test_build_config_provider_for_role() {
        let (_temp, manager) = create_test_manager();

        let config = manager.build_config(ConfigOverrides::default()).unwrap();

        // Main role should have provider
        let provider = config.provider_for_role(ModelRole::Main);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "test-openai");
    }

    #[test]
    fn test_build_config_with_model_overrides() {
        use cocode_protocol::model::ModelRoles;
        use cocode_protocol::model::ModelSpec;

        let (_temp_dir, manager) = create_test_manager();

        // First create the model roles override
        let mut models = ModelRoles::default();
        models.set(ModelRole::Main, ModelSpec::new("test-openai", "gpt-5-mini"));
        let overrides = ConfigOverrides::new().with_models(models);

        let config = manager.build_config(overrides).unwrap();

        // Main model should be the overridden one
        let main = config.main_model().unwrap();
        assert_eq!(main.model, "gpt-5-mini");
    }
}
