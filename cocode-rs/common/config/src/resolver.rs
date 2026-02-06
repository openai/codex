//! Configuration resolution and merging logic.
//!
//! This module implements the layered configuration resolution:
//!
//! **Precedence (highest to lowest):**
//! 1. Runtime overrides (API calls, `/model` command)
//! 2. Environment variables (for secrets)
//! 3. Model entry in provider config (flattened ModelInfo + model_options)
//! 4. User model config (`models.json`)
//! 5. Built-in defaults (compiled into binary)

use crate::builtin;
use crate::error::ConfigError;
use crate::error::NotFoundKind;
use crate::error::config_error::AuthSnafu;
use crate::error::config_error::ConfigValidationSnafu;
use crate::error::config_error::NotFoundSnafu;
use crate::types::ModelsFile;
use crate::types::ProviderConfig;
use crate::types::ProvidersFile;
use cocode_protocol::ModelInfo;
use cocode_protocol::ProviderInfo;
use cocode_protocol::ProviderModel;
use snafu::OptionExt;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Configuration resolver that merges layers of configuration.
#[derive(Debug, Clone)]
pub struct ConfigResolver {
    pub(crate) models: HashMap<String, ModelInfo>,
    pub(crate) providers: HashMap<String, ProviderConfig>,
    /// Config directory for resolving relative paths (e.g., base_instructions_file).
    pub(crate) config_dir: Option<PathBuf>,
}

impl ConfigResolver {
    /// Create a new resolver from loaded configuration.
    pub fn new(models_file: ModelsFile, providers_file: ProvidersFile) -> Self {
        Self {
            models: models_file.models,
            providers: providers_file.providers,
            config_dir: None,
        }
    }

    /// Create a new resolver with a config directory.
    pub fn with_config_dir(
        models_file: ModelsFile,
        providers_file: ProvidersFile,
        config_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            models: models_file.models,
            providers: providers_file.providers,
            config_dir: Some(config_dir.into()),
        }
    }

    /// Create an empty resolver (uses only built-in defaults).
    pub fn empty() -> Self {
        Self {
            models: HashMap::new(),
            providers: HashMap::new(),
            config_dir: None,
        }
    }

    /// Set the config directory for resolving relative paths.
    pub fn set_config_dir(&mut self, config_dir: impl Into<PathBuf>) {
        self.config_dir = Some(config_dir.into());
    }

    /// Resolve model info by merging all configuration layers.
    ///
    /// Resolution order (later overrides earlier):
    /// 1. Built-in defaults
    /// 2. User model config (models.json)
    /// 3. Model entry config (flattened ModelInfo fields)
    /// 4. Model entry options (merged into ModelInfo.options)
    ///
    /// # Arguments
    /// * `provider_name` - Provider identifier (e.g., "openai", "anthropic")
    /// * `slug` - Model configuration identifier (e.g., "gpt-4o", "deepseek-r1")
    pub fn resolve_model_info(
        &self,
        provider_name: &str,
        slug: &str,
    ) -> Result<ModelInfo, ConfigError> {
        // Get provider config, or use a default empty one
        let config = if let Some(provider_config) = self.providers.get(provider_name) {
            self.resolve_model_info_for_provider(provider_config, slug)
        } else {
            // No provider config, use defaults only
            self.resolve_model_info_no_provider(slug)
        };

        // Validate required fields
        if config.context_window.is_none() || config.max_output_tokens.is_none() {
            return ConfigValidationSnafu {
                file: format!("model:{slug}"),
                message: "context_window and max_output_tokens are required".to_string(),
            }
            .fail();
        }

        Ok(config)
    }

    /// Resolve model info without a provider config (fallback path).
    fn resolve_model_info_no_provider(&self, slug: &str) -> ModelInfo {
        // Start with built-in defaults
        let mut config = builtin::get_model_defaults(slug).unwrap_or_default();
        config.slug = slug.to_string();

        // Layer 2: User model config from models.json
        if let Some(user_config) = self.models.get(slug) {
            config.merge_from(user_config);
            debug!(slug = slug, "Applied user model config");
        }

        // Resolve base_instructions: file takes precedence over inline
        if let Some(resolved_instructions) = self.resolve_base_instructions(&config) {
            config.base_instructions = Some(resolved_instructions);
            config.base_instructions_file = None;
        }

        config
    }

    /// Resolve base_instructions from inline string or file.
    ///
    /// If `base_instructions_file` is set and the file exists, load its content.
    /// Otherwise, use the inline `base_instructions`.
    fn resolve_base_instructions(&self, config: &ModelInfo) -> Option<String> {
        // Try to load from file first if config_dir is set
        if let (Some(file_path), Some(config_dir)) =
            (&config.base_instructions_file, &self.config_dir)
        {
            let full_path = config_dir.join(file_path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        // Log the overwrite if inline instructions were also set
                        if config.base_instructions.is_some() {
                            info!(
                                file = %full_path.display(),
                                "Loaded base_instructions from file (overwriting inline)"
                            );
                        } else {
                            debug!(file = %full_path.display(), "Loaded base_instructions from file");
                        }
                        return Some(trimmed.to_string());
                    }
                }
                Err(e) => {
                    warn!(
                        file = %full_path.display(),
                        error = %e,
                        "Failed to read base_instructions_file"
                    );
                }
            }
        }

        // Fall back to inline instructions
        config.base_instructions.clone()
    }

    /// Resolve a model alias to its API model name.
    ///
    /// Returns the alias if set and non-empty, otherwise returns the slug.
    /// For example, slug "deepseek-r1" might return "ep-20250109-xxxxx".
    pub fn resolve_model_alias<'a>(&'a self, provider_name: &str, slug: &'a str) -> &'a str {
        self.providers
            .get(provider_name)
            .and_then(|p| p.find_model(slug))
            .map(|m| m.api_model_name())
            .unwrap_or(slug)
    }

    /// Resolve provider configuration into a complete `ProviderInfo`.
    ///
    /// This resolves:
    /// - API key from environment variables or config
    /// - All models with their resolved `ModelInfo`
    pub fn resolve_provider(&self, provider_name: &str) -> Result<ProviderInfo, ConfigError> {
        let provider_config = self.providers.get(provider_name).context(NotFoundSnafu {
            kind: NotFoundKind::Provider,
            name: provider_name.to_string(),
        })?;

        // Resolve API key: env var takes precedence
        let api_key = self.resolve_api_key(provider_config).ok_or_else(|| {
            let env_hint = provider_config
                .env_key
                .as_ref()
                .map(|k| format!(" (set {k} or api_key in config)"))
                .unwrap_or_default();
            AuthSnafu {
                message: format!("API key not found for provider '{provider_name}'{env_hint}"),
            }
            .build()
        })?;

        // Resolve all models for this provider
        let mut models = HashMap::new();
        for model_entry in &provider_config.models {
            let slug = model_entry.slug();
            // Build resolved ModelInfo
            let model_info = self.resolve_model_info_for_provider(provider_config, slug);

            // Create ProviderModel with model_alias preserved
            let provider_model = if let Some(alias) = &model_entry.model_alias {
                ProviderModel::with_alias(model_info, alias)
            } else {
                ProviderModel::new(model_info)
            };
            models.insert(slug.to_string(), provider_model);
        }

        let mut info = ProviderInfo::new(
            &provider_config.name,
            provider_config.provider_type,
            &provider_config.base_url,
        )
        .with_api_key(api_key)
        .with_timeout(provider_config.timeout_secs)
        .with_streaming(provider_config.streaming)
        .with_wire_api(provider_config.wire_api)
        .with_models(models);

        if let Some(extra) = &provider_config.options {
            info = info.with_options(extra.clone());
        }

        Ok(info)
    }

    /// Resolve and merge model config layers, returning a `ModelInfo`.
    ///
    /// This is used when building `ProviderInfo.models` to store fully resolved configs.
    ///
    /// # Arguments
    /// * `provider_config` - Provider configuration
    /// * `slug` - Model configuration identifier
    fn resolve_model_info_for_provider(
        &self,
        provider_config: &ProviderConfig,
        slug: &str,
    ) -> ModelInfo {
        // Start with built-in defaults
        let mut config = builtin::get_model_defaults(slug).unwrap_or_default();
        config.slug = slug.to_string();

        // Layer 2: User model config from models.json
        if let Some(user_config) = self.models.get(slug) {
            config.merge_from(user_config);
            debug!(slug = slug, "Applied user model config");
        }

        // Layer 3: Model entry config and options
        if let Some(model_entry) = provider_config.find_model(slug) {
            // Apply flattened ModelInfo fields
            config.merge_from(&model_entry.model_info);
            debug!(slug = slug, "Applied model entry config");

            // Merge model-specific options directly into ModelInfo.options
            if !model_entry.model_options.is_empty() {
                let opts = config.options.get_or_insert_with(HashMap::new);
                for (k, v) in &model_entry.model_options {
                    opts.insert(k.clone(), v.clone());
                }
                debug!(slug = slug, "Applied model-specific options");
            }
        }

        // Resolve timeout_secs: use model config or fall back to provider default
        if config.timeout_secs.is_none() {
            config.timeout_secs = Some(provider_config.timeout_secs);
        }

        // Resolve base_instructions: file takes precedence over inline
        if let Some(resolved_instructions) = self.resolve_base_instructions(&config) {
            config.base_instructions = Some(resolved_instructions);
            config.base_instructions_file = None; // Already resolved
        }

        config
    }

    /// Resolve API key from env var or config.
    fn resolve_api_key(&self, config: &ProviderConfig) -> Option<String> {
        // Try environment variable first
        if let Some(env_key) = &config.env_key {
            if let Ok(key) = env::var(env_key) {
                if !key.is_empty() {
                    debug!(env_key = env_key, "Resolved API key from environment");
                    return Some(key);
                }
            }
        }

        // Fall back to config
        config.api_key.clone()
    }

    /// Check if a provider is configured.
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// List all configured provider names.
    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }

    /// List models configured for a provider.
    pub fn list_models(&self, provider_name: &str) -> Vec<&str> {
        self.providers
            .get(provider_name)
            .map(|p| p.list_model_slugs())
            .unwrap_or_default()
    }

    /// Get provider config by name (for inspection).
    pub fn get_provider_config(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// Get model config by ID (for inspection).
    pub fn get_model_config(&self, id: &str) -> Option<&ModelInfo> {
        self.models.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProviderModelEntry;
    use crate::types::ProviderType;
    use crate::types::WireApi;
    use cocode_protocol::Capability;

    fn create_test_resolver() -> ConfigResolver {
        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelInfo {
                slug: "test-model".to_string(),
                display_name: Some("Test Model".to_string()),
                context_window: Some(8192),
                max_output_tokens: Some(2048),
                capabilities: Some(vec![Capability::TextGeneration, Capability::Streaming]),
                ..Default::default()
            },
        );
        models.insert(
            "deepseek-r1".to_string(),
            ModelInfo {
                slug: "deepseek-r1".to_string(),
                display_name: Some("DeepSeek R1".to_string()),
                context_window: Some(64000),
                max_output_tokens: Some(8192),
                ..Default::default()
            },
        );

        let mut providers = HashMap::new();
        providers.insert(
            "test-provider".to_string(),
            ProviderConfig {
                name: "Test Provider".to_string(),
                provider_type: ProviderType::Openai,
                base_url: "https://api.test.com".to_string(),
                timeout_secs: 300,
                env_key: Some("TEST_API_KEY".to_string()),
                api_key: Some("fallback-key".to_string()),
                streaming: true,
                wire_api: WireApi::Responses,
                models: vec![
                    ProviderModelEntry {
                        model_info: ModelInfo {
                            slug: "test-model".to_string(),
                            max_output_tokens: Some(4096), // Override
                            ..Default::default()
                        },
                        model_alias: None,
                        model_options: HashMap::new(),
                    },
                    ProviderModelEntry {
                        model_info: ModelInfo {
                            slug: "ep-12345".to_string(),
                            context_window: Some(32000),
                            max_output_tokens: Some(4096),
                            ..Default::default()
                        },
                        model_alias: Some("deepseek-r1".to_string()),
                        model_options: HashMap::new(),
                    },
                ],
                options: None,
                interceptors: Vec::new(),
            },
        );

        ConfigResolver {
            models,
            providers,
            config_dir: None,
        }
    }

    #[test]
    fn test_resolve_model_info_basic() {
        let resolver = create_test_resolver();
        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        assert_eq!(info.slug, "test-model");
        assert_eq!(info.display_name, Some("Test Model".to_string()));
        assert_eq!(info.context_window, Some(8192));
        // Provider model entry override applied
        assert_eq!(info.max_output_tokens, Some(4096));
    }

    #[test]
    fn test_resolve_model_alias() {
        let resolver = create_test_resolver();

        // Direct alias resolution
        let canonical = resolver.resolve_model_alias("test-provider", "ep-12345");
        assert_eq!(canonical, "deepseek-r1");

        // Non-aliased model returns itself
        let canonical = resolver.resolve_model_alias("test-provider", "test-model");
        assert_eq!(canonical, "test-model");
    }

    #[test]
    fn test_resolve_model_with_alias() {
        let resolver = create_test_resolver();
        let info = resolver
            .resolve_model_info("test-provider", "ep-12345")
            .unwrap();

        assert_eq!(info.slug, "ep-12345");
        // Model entry override applied
        assert_eq!(info.context_window, Some(32000));
    }

    #[test]
    fn test_resolve_provider_with_env_key() {
        let resolver = create_test_resolver();

        // Set env var
        // SAFETY: This is a test, and we're using a unique env var name
        unsafe {
            env::set_var("TEST_API_KEY", "env-api-key");
        }

        let config = resolver.resolve_provider("test-provider").unwrap();
        assert_eq!(config.api_key, "env-api-key");
        assert!(config.streaming);
        assert_eq!(config.wire_api, WireApi::Responses);

        // Clean up
        // SAFETY: This is a test cleanup
        unsafe {
            env::remove_var("TEST_API_KEY");
        }
    }

    #[test]
    fn test_resolve_provider_fallback_to_config() {
        let resolver = create_test_resolver();

        // Ensure env var is not set
        // SAFETY: This is a test cleanup
        unsafe {
            env::remove_var("TEST_API_KEY");
        }

        let config = resolver.resolve_provider("test-provider").unwrap();
        assert_eq!(config.api_key, "fallback-key");
    }

    #[test]
    fn test_resolve_provider_not_found() {
        use crate::error::NotFoundKind;
        let resolver = create_test_resolver();
        let result = resolver.resolve_provider("nonexistent");
        assert!(matches!(
            result,
            Err(ConfigError::NotFound {
                kind: NotFoundKind::Provider,
                ..
            })
        ));
    }

    #[test]
    fn test_list_providers() {
        let resolver = create_test_resolver();
        let providers = resolver.list_providers();
        assert!(providers.contains(&"test-provider"));
    }

    #[test]
    fn test_list_models() {
        let resolver = create_test_resolver();
        let models = resolver.list_models("test-provider");
        assert!(models.contains(&"test-model"));
        assert!(models.contains(&"ep-12345"));
    }

    #[test]
    fn test_empty_resolver() {
        let resolver = ConfigResolver::empty();
        assert!(resolver.list_providers().is_empty());
    }

    #[test]
    fn test_unknown_model_missing_required_fields() {
        // Unknown model without context_window/max_output_tokens should fail validation
        let resolver = create_test_resolver();
        let result = resolver.resolve_model_info("test-provider", "unknown-model");
        assert!(result.is_err());
    }

    #[test]
    fn test_base_instructions_from_file() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let instructions_content = "You are a helpful assistant.";
        std::fs::write(
            temp_dir.path().join("instructions.md"),
            instructions_content,
        )
        .unwrap();

        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelInfo {
                slug: "test-model".to_string(),
                display_name: Some("Test Model".to_string()),
                context_window: Some(4096),
                max_output_tokens: Some(1024),
                base_instructions_file: Some("instructions.md".to_string()),
                ..Default::default()
            },
        );

        let resolver = ConfigResolver {
            models,
            providers: HashMap::new(),
            config_dir: Some(temp_dir.path().to_path_buf()),
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        assert_eq!(
            info.base_instructions,
            Some(instructions_content.to_string())
        );
    }

    #[test]
    fn test_base_instructions_file_overrides_inline() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let file_content = "Instructions from file";
        std::fs::write(temp_dir.path().join("instructions.md"), file_content).unwrap();

        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelInfo {
                slug: "test-model".to_string(),
                display_name: Some("Test Model".to_string()),
                context_window: Some(4096),
                max_output_tokens: Some(1024),
                base_instructions: Some("Inline instructions".to_string()),
                base_instructions_file: Some("instructions.md".to_string()),
                ..Default::default()
            },
        );

        let resolver = ConfigResolver {
            models,
            providers: HashMap::new(),
            config_dir: Some(temp_dir.path().to_path_buf()),
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        // File should take precedence over inline
        assert_eq!(info.base_instructions, Some(file_content.to_string()));
    }

    #[test]
    fn test_base_instructions_fallback_to_inline() {
        let mut models = HashMap::new();
        models.insert(
            "test-model".to_string(),
            ModelInfo {
                slug: "test-model".to_string(),
                display_name: Some("Test Model".to_string()),
                context_window: Some(4096),
                max_output_tokens: Some(1024),
                base_instructions: Some("Inline instructions".to_string()),
                base_instructions_file: Some("nonexistent.md".to_string()),
                ..Default::default()
            },
        );

        let resolver = ConfigResolver {
            models,
            providers: HashMap::new(),
            config_dir: Some(PathBuf::from("/tmp")),
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        // Should fall back to inline when file doesn't exist
        assert_eq!(
            info.base_instructions,
            Some("Inline instructions".to_string())
        );
    }

    #[test]
    fn test_model_entry_options_merged() {
        // model_options on ProviderModelEntry are merged into ModelInfo.options
        let mut providers = HashMap::new();
        let mut model_opts = HashMap::new();
        model_opts.insert("temperature".to_string(), serde_json::json!(0.9));
        model_opts.insert("seed".to_string(), serde_json::json!(42));

        providers.insert(
            "test-provider".to_string(),
            ProviderConfig {
                name: "Test Provider".to_string(),
                provider_type: ProviderType::Openai,
                base_url: "https://api.test.com".to_string(),
                timeout_secs: 300,
                env_key: None,
                api_key: Some("test-key".to_string()),
                streaming: true,
                wire_api: WireApi::Responses,
                models: vec![ProviderModelEntry {
                    model_info: ModelInfo {
                        slug: "test-model".to_string(),
                        context_window: Some(4096),
                        max_output_tokens: Some(1024),
                        ..Default::default()
                    },
                    model_alias: None,
                    model_options: model_opts,
                }],
                options: None,
                interceptors: Vec::new(),
            },
        );

        let resolver = ConfigResolver {
            models: HashMap::new(),
            providers,
            config_dir: None,
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        // model_options are merged into ModelInfo.options
        assert!(info.options.is_some());
        let opts = info.options.unwrap();
        assert_eq!(opts.get("temperature"), Some(&serde_json::json!(0.9)));
        assert_eq!(opts.get("seed"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_resolve_provider_with_models() {
        let resolver = create_test_resolver();

        // Ensure env var is not set (use fallback key)
        // SAFETY: This is a test cleanup
        unsafe {
            env::remove_var("TEST_API_KEY");
        }

        let provider_info = resolver.resolve_provider("test-provider").unwrap();

        // Check provider fields
        assert_eq!(provider_info.name, "Test Provider");
        assert_eq!(provider_info.provider_type, ProviderType::Openai);
        assert_eq!(provider_info.base_url, "https://api.test.com");
        assert_eq!(provider_info.api_key, "fallback-key");
        assert_eq!(provider_info.timeout_secs, 300);
        assert!(provider_info.streaming);
        assert_eq!(provider_info.wire_api, WireApi::Responses);
        assert!(provider_info.has_api_key());

        // Check models are populated
        assert_eq!(provider_info.models.len(), 2);

        // Check model slugs
        let slugs = provider_info.model_slugs();
        assert!(slugs.contains(&"test-model"));
        assert!(slugs.contains(&"ep-12345"));

        // Check get_model returns ProviderModel
        let test_model = provider_info.get_model("test-model").unwrap();
        assert_eq!(test_model.slug(), "test-model");
        assert_eq!(test_model.info.display_name, Some("Test Model".to_string()));
        assert_eq!(test_model.info.max_output_tokens, Some(4096)); // Override applied
        assert!(test_model.model_alias.is_none()); // No alias for this model

        // Check ep-12345 has model_alias
        let ep_model = provider_info.get_model("ep-12345").unwrap();
        assert_eq!(ep_model.slug(), "ep-12345");
        assert_eq!(ep_model.model_alias, Some("deepseek-r1".to_string()));
        assert_eq!(ep_model.api_model_name(), "deepseek-r1"); // Returns alias

        // Check api_model_name helper on ProviderInfo
        assert_eq!(
            provider_info.api_model_name("test-model"),
            Some("test-model")
        ); // No alias
        assert_eq!(
            provider_info.api_model_name("ep-12345"),
            Some("deepseek-r1")
        ); // Has alias
        assert_eq!(provider_info.api_model_name("nonexistent"), None);

        // Check effective_timeout
        assert_eq!(provider_info.effective_timeout("test-model"), 300); // Provider default (no model override)
        assert_eq!(provider_info.effective_timeout("ep-12345"), 300); // Provider default
        assert_eq!(provider_info.effective_timeout("nonexistent"), 300); // Provider default for unknown
    }

    #[test]
    fn test_options_field_propagation() {
        // Test that options fields are properly merged through resolution layers
        let mut models = HashMap::new();
        let mut user_opts = HashMap::new();
        user_opts.insert("user_key".to_string(), serde_json::json!("user_value"));
        user_opts.insert(
            "override_key".to_string(),
            serde_json::json!("user_override"),
        );

        models.insert(
            "test-model".to_string(),
            ModelInfo {
                slug: "test-model".to_string(),
                context_window: Some(4096),
                max_output_tokens: Some(1024),
                options: Some(user_opts),
                ..Default::default()
            },
        );

        let mut providers = HashMap::new();
        let mut model_opts = HashMap::new();
        model_opts.insert("model_key".to_string(), serde_json::json!("model_value"));
        model_opts.insert(
            "override_key".to_string(),
            serde_json::json!("model_override"),
        ); // Should override user_override

        providers.insert(
            "test-provider".to_string(),
            ProviderConfig {
                name: "Test Provider".to_string(),
                provider_type: ProviderType::Openai,
                base_url: "https://api.test.com".to_string(),
                timeout_secs: 300,
                env_key: None,
                api_key: Some("test-key".to_string()),
                streaming: true,
                wire_api: WireApi::Responses,
                models: vec![ProviderModelEntry {
                    model_info: ModelInfo {
                        slug: "test-model".to_string(),
                        options: Some(model_opts),
                        ..Default::default()
                    },
                    model_alias: None,
                    model_options: HashMap::new(),
                }],
                options: None,
                interceptors: Vec::new(),
            },
        );

        let resolver = ConfigResolver {
            models,
            providers,
            config_dir: None,
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        // Options should be present
        assert!(info.options.is_some());
        let opts = info.options.unwrap();

        // User key preserved
        assert_eq!(opts.get("user_key"), Some(&serde_json::json!("user_value")));
        // Model key added
        assert_eq!(
            opts.get("model_key"),
            Some(&serde_json::json!("model_value"))
        );
        // Model override takes precedence over user
        assert_eq!(
            opts.get("override_key"),
            Some(&serde_json::json!("model_override"))
        );
    }

    #[test]
    fn test_model_options_go_to_options() {
        // ProviderModelEntry.model_options are merged into ModelInfo.options
        let mut providers = HashMap::new();
        let mut model_options = HashMap::new();
        model_options.insert(
            "response_format".to_string(),
            serde_json::json!({"type": "json_object"}),
        );
        model_options.insert("seed".to_string(), serde_json::json!(42));

        providers.insert(
            "test-provider".to_string(),
            ProviderConfig {
                name: "Test Provider".to_string(),
                provider_type: ProviderType::Openai,
                base_url: "https://api.test.com".to_string(),
                timeout_secs: 300,
                env_key: None,
                api_key: Some("test-key".to_string()),
                streaming: true,
                wire_api: WireApi::Responses,
                models: vec![ProviderModelEntry {
                    model_info: ModelInfo {
                        slug: "test-model".to_string(),
                        context_window: Some(4096),
                        max_output_tokens: Some(1024),
                        ..Default::default()
                    },
                    model_alias: None,
                    model_options,
                }],
                options: None,
                interceptors: Vec::new(),
            },
        );

        let resolver = ConfigResolver {
            models: HashMap::new(),
            providers,
            config_dir: None,
        };

        let info = resolver
            .resolve_model_info("test-provider", "test-model")
            .unwrap();

        // model_options go to ModelInfo.options
        assert!(info.options.is_some());
        let opts = info.options.unwrap();
        assert_eq!(
            opts.get("response_format"),
            Some(&serde_json::json!({"type": "json_object"}))
        );
        assert_eq!(opts.get("seed"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_required_fields_validation() {
        // Model without context_window should fail
        let mut models = HashMap::new();
        models.insert(
            "no-context".to_string(),
            ModelInfo {
                slug: "no-context".to_string(),
                max_output_tokens: Some(1024),
                ..Default::default()
            },
        );

        let resolver = ConfigResolver {
            models,
            providers: HashMap::new(),
            config_dir: None,
        };

        let result = resolver.resolve_model_info("any-provider", "no-context");
        assert!(result.is_err());
    }
}
