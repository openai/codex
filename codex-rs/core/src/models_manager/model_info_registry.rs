//! Global registry for model info configurations.
//!
//! This module provides a registry that combines code-defined model info
//! with user-defined info from `~/.codex/model_info.toml`.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;

use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationPolicyConfig;

use super::model_info::find_model_info_for_slug;
use super::model_info_config::ModelInfoConfig;

/// Name of the model info configuration file.
pub const MODEL_INFO_TOML: &str = "model_info.toml";

/// Global model info registry.
static INFO_REGISTRY: OnceLock<RwLock<ModelInfoRegistry>> = OnceLock::new();

/// Registry for model info configurations.
///
/// Combines user-defined info from TOML with code-defined defaults.
#[derive(Debug)]
pub struct ModelInfoRegistry {
    /// User-configured info loaded from model_info.toml.
    user_info: HashMap<String, ModelInfoConfig>,
    /// Path to codex home directory for resolving relative paths.
    codex_home: PathBuf,
}

impl Default for ModelInfoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelInfoRegistry {
    /// Get the global registry instance.
    pub fn global() -> &'static RwLock<Self> {
        INFO_REGISTRY.get_or_init(|| RwLock::new(Self::new()))
    }

    /// Create a new empty registry.
    fn new() -> Self {
        Self {
            user_info: HashMap::new(),
            codex_home: PathBuf::new(),
        }
    }

    /// Load user-defined model info from configuration file.
    ///
    /// Reads `~/.codex/model_info.toml` and populates the registry.
    /// Does nothing if the file doesn't exist.
    pub fn load_from_file(&mut self, codex_home: &Path) -> std::io::Result<()> {
        self.codex_home = codex_home.to_path_buf();
        let config_path = codex_home.join(MODEL_INFO_TOML);

        if !config_path.exists() {
            tracing::debug!("Model info config not found at {}", config_path.display());
            return Ok(());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let info: HashMap<String, ModelInfoConfig> = toml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        tracing::info!(
            "Loaded {} custom model info from {}",
            info.len(),
            config_path.display()
        );

        self.user_info = info;
        Ok(())
    }

    /// Resolve a model info by ID, returning ModelInfo.
    ///
    /// Resolution priority:
    /// 1. User-defined info from model_info.toml
    /// 2. Code-defined info via `find_model_info_for_slug()`
    pub fn resolve(&self, model_id: &str) -> ModelInfo {
        // Check user-configured info first
        if let Some(config) = self.user_info.get(model_id) {
            return self.build_from_config(model_id, config);
        }

        // Fall back to code-defined model info
        find_model_info_for_slug(model_id)
    }

    /// Build a ModelInfo from user configuration.
    fn build_from_config(&self, id: &str, config: &ModelInfoConfig) -> ModelInfo {
        // Start with default model info from code-defined lookup
        let mut info = find_model_info_for_slug(id);

        // Apply user configuration overrides
        info.slug = id.to_string();

        if let Some(display_name) = &config.display_name {
            info.display_name = display_name.clone();
        }

        if let Some(context_window) = config.context_window {
            info.context_window = Some(context_window);
        }

        if let Some(auto_compact) = config.auto_compact_token_limit {
            info.auto_compact_token_limit = Some(auto_compact);
        }

        if config.supports_reasoning_summaries {
            info.supports_reasoning_summaries = true;
        }

        if config.supports_parallel_tool_calls {
            info.supports_parallel_tool_calls = true;
        }

        if let Some(effort) = &config.default_reasoning_effort {
            info.default_reasoning_level = Some(effort.clone());
        }

        // Resolve base_instructions (inline or file)
        if let Some(instructions) = config.resolve_base_instructions(&self.codex_home) {
            info.base_instructions = instructions;
        }

        info
    }

    /// Check if a model ID exists in user configuration.
    pub fn has_user_info(&self, model_id: &str) -> bool {
        self.user_info.contains_key(model_id)
    }
}

/// Resolve a model info by ID using the global registry.
///
/// This is the main entry point for model info resolution.
///
/// # Resolution Priority
/// 1. User-defined info from `~/.codex/model_info.toml`
/// 2. Code-defined info via `find_model_info_for_slug()`
pub fn resolve_model_info(model_id: &str) -> ModelInfo {
    ModelInfoRegistry::global()
        .read()
        .expect("model info registry lock poisoned")
        .resolve(model_id)
}

/// Initialize the global registry with info from the config file.
///
/// Should be called early during application startup.
pub fn init_registry(codex_home: &Path) -> std::io::Result<()> {
    let mut registry = ModelInfoRegistry::global()
        .write()
        .expect("model info registry lock poisoned");
    registry.load_from_file(codex_home)
}

/// Create a default ModelInfo for an unknown model slug.
pub fn derive_default_model_info(slug: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: vec![],
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 0,
        upgrade: None,
        base_instructions: String::new(),
        model_instructions_template: None,
        supports_reasoning_summaries: true,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: true,
        context_window: Some(128_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_falls_back_to_code_defined() {
        let registry = ModelInfoRegistry::new();

        // Should fall back to code-defined model info
        let info = registry.resolve("gpt-5.1-codex-max");
        assert_eq!(info.slug, "gpt-5.1-codex-max");
        // Code-defined gpt-5.1-codex-max has reasoning summaries enabled
        assert!(info.supports_reasoning_summaries);
    }

    #[test]
    fn test_user_config_takes_precedence() {
        let codex_home = tempdir().unwrap();

        // Write a user config
        let config_content = r#"
[custom-model]
display_name = "Custom Model"
context_window = 32000
supports_reasoning_summaries = true
base_instructions = "Custom instructions"
"#;
        std::fs::write(codex_home.path().join(MODEL_INFO_TOML), config_content).unwrap();

        let mut registry = ModelInfoRegistry::new();
        registry.load_from_file(codex_home.path()).unwrap();

        let info = registry.resolve("custom-model");
        assert_eq!(info.slug, "custom-model");
        assert_eq!(info.context_window, Some(32000));
        assert!(info.supports_reasoning_summaries);
        assert_eq!(info.base_instructions, "Custom instructions");
    }

    #[test]
    fn test_missing_config_file_is_ok() {
        let codex_home = tempdir().unwrap();
        let mut registry = ModelInfoRegistry::new();

        // Should not error if file doesn't exist
        let result = registry.load_from_file(codex_home.path());
        assert!(result.is_ok());
        assert!(registry.user_info.is_empty());
    }
}
