//! Configuration file loading.
//!
//! This module handles loading configuration from JSON files in the config directory.
//!
//! # Multi-file Support
//!
//! Models and providers support loading from multiple files:
//! - `*model.json` - Model definitions (e.g., `gpt_model.json`, `google_model.json`, `model.json`)
//! - `*provider.json` - Provider configurations (e.g., `test_provider.json`, `provider.json`)
//!
//! Files are loaded in alphabetical order and merged. Duplicate slugs/names are an error.

use crate::error::ConfigError;
use crate::error::config_error::IoSnafu;
use crate::error::config_error::JsonParseSnafu;
use crate::error::config_error::JsoncParseSnafu;
use crate::json_config::AppConfig;
use crate::types::ModelsFile;
use crate::types::ProviderConfig;
use crate::types::ProvidersFile;
use cocode_protocol::ModelInfo;
use jsonc_parser::ParseOptions;
use snafu::ResultExt;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;

/// Default configuration directory path.
pub const DEFAULT_CONFIG_DIR: &str = ".cocode";

/// Application configuration file name (JSON).
pub const CONFIG_FILE: &str = "config.json";

/// Legacy constant for backwards compatibility.
#[deprecated(since = "0.1.0", note = "Use CONFIG_FILE instead")]
pub const CONFIG_TOML_FILE: &str = "config.json";

/// Instruction file names.
pub const AGENTS_MD_FILE: &str = "AGENTS.md";

/// Log directory name.
pub const LOG_DIR_NAME: &str = "log";

/// Environment variable for custom cocode home directory.
pub const COCODE_HOME_ENV: &str = "COCODE_HOME";

/// Environment variable for custom log directory.
pub const COCODE_LOG_DIR_ENV: &str = "COCODE_LOG_DIR";

/// Get the default configuration directory path.
///
/// Returns `~/.cocode` on Unix systems and `%USERPROFILE%\.cocode` on Windows.
pub fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_CONFIG_DIR)
}

/// Find the cocode home directory.
///
/// Checks `COCODE_HOME` environment variable first, then falls back to
/// the default config directory (`~/.cocode`).
///
/// If `COCODE_HOME` is a relative path, it's resolved relative to the
/// current working directory.
pub fn find_cocode_home() -> PathBuf {
    if let Ok(custom_home) = std::env::var(COCODE_HOME_ENV) {
        let path = PathBuf::from(&custom_home);
        if path.is_absolute() {
            return path;
        }
        std::env::current_dir()
            .map(|cwd| cwd.join(&custom_home))
            .unwrap_or_else(|_| PathBuf::from(custom_home))
    } else {
        default_config_dir()
    }
}

/// Get the log directory path.
///
/// Checks `COCODE_LOG_DIR` environment variable first, then falls back to
/// `{cocode_home}/log`.
///
/// If `COCODE_LOG_DIR` is a relative path, it's resolved relative to the
/// current working directory.
pub fn log_dir() -> PathBuf {
    if let Ok(custom_log_dir) = std::env::var(COCODE_LOG_DIR_ENV) {
        let path = PathBuf::from(&custom_log_dir);
        if path.is_absolute() {
            return path;
        }
        std::env::current_dir()
            .map(|cwd| cwd.join(&custom_log_dir))
            .unwrap_or_else(|_| PathBuf::from(custom_log_dir))
    } else {
        find_cocode_home().join(LOG_DIR_NAME)
    }
}

/// Load instructions from a project directory.
///
/// Looks for instruction files in the following order:
/// 1. `AGENTS.md`
///
/// Returns `None` if no instruction file is found or if the file is empty.
pub fn load_instructions(project_dir: &Path) -> Option<String> {
    let candidates = [AGENTS_MD_FILE];
    for name in candidates {
        let path = project_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Configuration loader for JSON files.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    config_dir: PathBuf,
}

impl ConfigLoader {
    /// Create a loader for the default config directory (~/.cocode).
    pub fn default() -> Self {
        Self {
            config_dir: default_config_dir(),
        }
    }

    /// Create a loader for a specific config directory.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_dir: path.as_ref().to_path_buf(),
        }
    }

    /// Get the config directory path.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Check if the config directory exists.
    pub fn exists(&self) -> bool {
        self.config_dir.exists()
    }

    /// Ensure the config directory exists, creating it if necessary.
    pub fn ensure_dir(&self) -> Result<(), ConfigError> {
        if !self.config_dir.exists() {
            std::fs::create_dir_all(&self.config_dir).context(IoSnafu {
                message: format!(
                    "Failed to create config directory {}",
                    self.config_dir.display(),
                ),
            })?;
            debug!(path = %self.config_dir.display(), "Created config directory");
        }
        Ok(())
    }

    /// Find all config files matching a suffix pattern.
    ///
    /// Returns files matching `*{suffix}.json` in the config directory,
    /// sorted alphabetically for deterministic merge order.
    fn find_config_files(&self, suffix: &str) -> Vec<PathBuf> {
        if !self.config_dir.exists() {
            return Vec::new();
        }

        let pattern = format!("{suffix}.json");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.config_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|name| name.ends_with(&pattern))
            })
            .collect();

        files.sort();
        files
    }

    /// Load models from all `*model.json` files.
    ///
    /// Files are loaded in alphabetical order and merged.
    /// Returns an error if duplicate model slugs are found across files.
    pub fn load_models(&self) -> Result<ModelsFile, ConfigError> {
        let files = self.find_config_files("model");
        if files.is_empty() {
            debug!("No model config files found, using defaults");
            return Ok(ModelsFile::default());
        }

        let mut merged = ModelsFile::default();
        for path in files {
            let list: Vec<ModelInfo> = self.load_json_file(&path)?;
            debug!(path = %path.display(), count = list.len(), "Loaded model file");
            merged.add_models(list, path.display())?;
        }

        Ok(merged)
    }

    /// Load providers from all `*provider.json` files.
    ///
    /// Files are loaded in alphabetical order and merged.
    /// Returns an error if duplicate provider names are found across files.
    pub fn load_providers(&self) -> Result<ProvidersFile, ConfigError> {
        let files = self.find_config_files("provider");
        if files.is_empty() {
            debug!("No provider config files found, using defaults");
            return Ok(ProvidersFile::default());
        }

        let mut merged = ProvidersFile::default();
        for path in files {
            let list: Vec<ProviderConfig> = self.load_json_file(&path)?;
            debug!(path = %path.display(), count = list.len(), "Loaded provider file");
            merged.add_providers(list, path.display())?;
        }

        Ok(merged)
    }

    /// Load the application configuration file (config.json).
    pub fn load_config(&self) -> Result<AppConfig, ConfigError> {
        let path = self.config_dir.join(CONFIG_FILE);
        self.load_json_file(&path)
    }

    /// Load a JSON/JSONC file, returning default if it doesn't exist.
    ///
    /// Supports JSONC extensions:
    /// - Comments (`//` and `/* */`)
    /// - Trailing commas (`[1, 2, 3,]`)
    /// - Unquoted keys (`{key: "value"}`) - only simple alphanumeric names
    fn load_json_file<T: serde::de::DeserializeOwned + Default>(
        &self,
        path: &Path,
    ) -> Result<T, ConfigError> {
        if !path.exists() {
            debug!(path = %path.display(), "Config file not found, using defaults");
            return Ok(T::default());
        }

        let content = std::fs::read_to_string(path).context(IoSnafu {
            message: format!("Failed to read {}", path.display()),
        })?;

        // Handle empty files
        if content.trim().is_empty() {
            debug!(path = %path.display(), "Config file is empty, using defaults");
            return Ok(T::default());
        }

        // Parse with JSONC extensions enabled
        let parse_opts = ParseOptions {
            allow_comments: true,
            allow_trailing_commas: true,
            allow_loose_object_property_names: true,
        };

        let json_value =
            jsonc_parser::parse_to_serde_value(&content, &parse_opts).map_err(|e| {
                JsoncParseSnafu {
                    file: path.display().to_string(),
                    message: e.to_string(),
                }
                .build()
            })?;

        // parse_to_serde_value returns Option<Value>, None means empty/whitespace-only
        let json_value = json_value.unwrap_or(serde_json::Value::Null);

        serde_json::from_value(json_value).context(JsonParseSnafu {
            file: path.display().to_string(),
        })
    }

    /// Load all configuration files at once.
    ///
    /// Returns an error if any configuration file has invalid JSON format or
    /// fails validation (e.g., duplicate provider names). This ensures users
    /// are notified of configuration errors rather than silently using defaults.
    ///
    /// Note: Missing or empty configuration files are handled gracefully by
    /// `load_json_file()` which returns `T::default()` in those cases.
    pub fn load_all(&self) -> Result<LoadedConfig, ConfigError> {
        let models = self.load_models()?;
        let providers = self.load_providers()?;
        let config = self.load_config()?;

        Ok(LoadedConfig {
            models,
            providers,
            config,
        })
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::default()
    }
}

/// All loaded configuration data.
#[derive(Debug, Clone, Default)]
pub struct LoadedConfig {
    /// Models configuration (merged from all *model.json files).
    pub models: ModelsFile,
    /// Providers configuration (merged from all *provider.json files).
    pub providers: ProvidersFile,
    /// Application configuration (from config.json).
    pub config: AppConfig,
}

impl LoadedConfig {
    /// Create empty loaded config.
    pub fn empty() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_config() -> (TempDir, ConfigLoader) {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::from_path(temp_dir.path());
        (temp_dir, loader)
    }

    #[test]
    fn test_default_config_dir() {
        let dir = default_config_dir();
        assert!(dir.to_string_lossy().contains(".cocode"));
    }

    #[test]
    fn test_loader_nonexistent_dir() {
        let loader = ConfigLoader::from_path("/nonexistent/path");
        assert!(!loader.exists());

        // Should return defaults for missing files
        let models = loader.load_models().unwrap();
        assert!(models.models.is_empty());
    }

    #[test]
    fn test_loader_ensure_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config");
        let loader = ConfigLoader::from_path(&config_path);

        assert!(!config_path.exists());
        loader.ensure_dir().unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_load_single_model_file() {
        let (temp_dir, loader) = create_temp_config();

        // New list format for *model.json files
        let models_json = r#"[
            {
                "slug": "test-model",
                "display_name": "Test Model",
                "context_window": 4096
            }
        ]"#;

        let models_path = temp_dir.path().join("model.json");
        std::fs::write(&models_path, models_json).unwrap();

        let models = loader.load_models().unwrap();
        assert!(models.models.contains_key("test-model"));
        assert_eq!(
            models.models["test-model"].display_name,
            Some("Test Model".to_string())
        );
    }

    #[test]
    fn test_load_multiple_model_files() {
        let (temp_dir, loader) = create_temp_config();

        // First file: gpt_model.json
        let gpt_models = r#"[
            {"slug": "gpt-5", "display_name": "GPT-5", "context_window": 128000},
            {"slug": "gpt-5-mini", "display_name": "GPT-5 Mini", "context_window": 32000}
        ]"#;
        std::fs::write(temp_dir.path().join("gpt_model.json"), gpt_models).unwrap();

        // Second file: claude_model.json
        let claude_models = r#"[
            {"slug": "claude-opus", "display_name": "Claude Opus", "context_window": 200000}
        ]"#;
        std::fs::write(temp_dir.path().join("claude_model.json"), claude_models).unwrap();

        let models = loader.load_models().unwrap();
        assert_eq!(models.models.len(), 3);
        assert!(models.models.contains_key("gpt-5"));
        assert!(models.models.contains_key("gpt-5-mini"));
        assert!(models.models.contains_key("claude-opus"));
    }

    #[test]
    fn test_load_model_files_duplicate_error() {
        let (temp_dir, loader) = create_temp_config();

        // First file
        let file1 = r#"[{"slug": "gpt-5", "display_name": "GPT-5"}]"#;
        std::fs::write(temp_dir.path().join("a_model.json"), file1).unwrap();

        // Second file with same slug
        let file2 = r#"[{"slug": "gpt-5", "display_name": "GPT-5 Duplicate"}]"#;
        std::fs::write(temp_dir.path().join("b_model.json"), file2).unwrap();

        let result = loader.load_models();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ConfigValidation { .. }));
        assert!(err.to_string().contains("duplicate model slug"));
    }

    #[test]
    fn test_load_single_provider_file() {
        let (temp_dir, loader) = create_temp_config();

        // New list format for *provider.json files
        let providers_json = r#"[
            {
                "name": "openai",
                "type": "openai",
                "base_url": "https://api.openai.com/v1",
                "env_key": "OPENAI_API_KEY",
                "models": []
            }
        ]"#;

        let providers_path = temp_dir.path().join("provider.json");
        std::fs::write(&providers_path, providers_json).unwrap();

        let providers = loader.load_providers().unwrap();
        assert!(providers.providers.contains_key("openai"));
    }

    #[test]
    fn test_load_multiple_provider_files() {
        let (temp_dir, loader) = create_temp_config();

        // First file
        let file1 = r#"[
            {"name": "openai", "type": "openai", "base_url": "https://api.openai.com/v1", "models": []}
        ]"#;
        std::fs::write(temp_dir.path().join("openai_provider.json"), file1).unwrap();

        // Second file
        let file2 = r#"[
            {"name": "anthropic", "type": "anthropic", "base_url": "https://api.anthropic.com", "models": []}
        ]"#;
        std::fs::write(temp_dir.path().join("anthropic_provider.json"), file2).unwrap();

        let providers = loader.load_providers().unwrap();
        assert_eq!(providers.providers.len(), 2);
        assert!(providers.providers.contains_key("openai"));
        assert!(providers.providers.contains_key("anthropic"));
    }

    #[test]
    fn test_load_provider_files_duplicate_error() {
        let (temp_dir, loader) = create_temp_config();

        // First file
        let file1 = r#"[{"name": "openai", "type": "openai", "base_url": "https://api.openai.com/v1", "models": []}]"#;
        std::fs::write(temp_dir.path().join("a_provider.json"), file1).unwrap();

        // Second file with same name
        let file2 = r#"[{"name": "openai", "type": "openai", "base_url": "https://other.com/v1", "models": []}]"#;
        std::fs::write(temp_dir.path().join("b_provider.json"), file2).unwrap();

        let result = loader.load_providers();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ConfigValidation { .. }));
        assert!(err.to_string().contains("duplicate provider name"));
    }

    #[test]
    fn test_load_config_json() {
        let (temp_dir, loader) = create_temp_config();

        let config_json = r#"{
            "models": {
                "main": "openai/gpt-5"
            },
            "profile": "fast"
        }"#;
        std::fs::write(temp_dir.path().join(CONFIG_FILE), config_json).unwrap();

        let config = loader.load_config().unwrap();
        assert!(config.models.is_some());
        let models = config.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().provider, "openai");
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5");
        assert_eq!(config.profile, Some("fast".to_string()));
    }

    #[test]
    fn test_load_all() {
        let (temp_dir, loader) = create_temp_config();

        // Create model file
        let models = r#"[{"slug": "test-model", "display_name": "Test"}]"#;
        std::fs::write(temp_dir.path().join("model.json"), models).unwrap();

        // Create provider file
        let providers =
            r#"[{"name": "test", "type": "openai", "base_url": "https://test.com", "models": []}]"#;
        std::fs::write(temp_dir.path().join("provider.json"), providers).unwrap();

        // Create config file
        let config = r#"{"models": {"main": "test/test-model"}}"#;
        std::fs::write(temp_dir.path().join(CONFIG_FILE), config).unwrap();

        let loaded = loader.load_all().unwrap();
        assert!(loaded.models.models.contains_key("test-model"));
        assert!(loaded.providers.providers.contains_key("test"));
        assert!(loaded.config.models.is_some());
    }

    #[test]
    fn test_load_empty_model_file() {
        let (temp_dir, loader) = create_temp_config();

        let models_path = temp_dir.path().join("model.json");
        std::fs::write(&models_path, "").unwrap();

        // Empty file should return empty list (default)
        let models = loader.load_models().unwrap();
        assert!(models.models.is_empty());
    }

    #[test]
    fn test_load_invalid_json() {
        let (temp_dir, loader) = create_temp_config();

        let models_path = temp_dir.path().join("model.json");
        std::fs::write(&models_path, "{ invalid json }").unwrap();

        let result = loader.load_models();
        assert!(result.is_err());
        let err = result.unwrap_err();
        // With JSONC parser, unquoted keys are allowed, so this parses differently
        // The error is now from serde deserialization, not JSON parsing
        assert!(
            matches!(err, ConfigError::JsonParse { .. })
                || matches!(err, ConfigError::JsoncParse { .. })
        );
    }

    #[test]
    fn test_load_jsonc_with_comments() {
        let (temp_dir, loader) = create_temp_config();

        // JSONC content with comments and trailing commas
        let jsonc_content = r#"[
            // This is a line comment
            {
                "slug": "test-model",
                "display_name": "Test Model", // inline comment
                "context_window": 4096,  // trailing comma allowed
            },
            /* Block comment */
        ]"#;

        std::fs::write(temp_dir.path().join("model.json"), jsonc_content).unwrap();

        let models = loader.load_models().unwrap();
        assert!(models.models.contains_key("test-model"));
        assert_eq!(
            models.models["test-model"].display_name,
            Some("Test Model".to_string())
        );
        assert_eq!(models.models["test-model"].context_window, Some(4096));
    }

    #[test]
    fn test_load_jsonc_with_unquoted_keys() {
        let (temp_dir, loader) = create_temp_config();

        // JSONC content with unquoted keys (only simple alphanumeric names work)
        // Note: underscores in unquoted keys are not supported by jsonc-parser 0.24
        let jsonc_content = r#"[
            {
                slug: "unquoted-model"
            }
        ]"#;

        std::fs::write(temp_dir.path().join("model.json"), jsonc_content).unwrap();

        let models = loader.load_models().unwrap();
        assert!(models.models.contains_key("unquoted-model"));
    }

    #[test]
    fn test_load_jsonc_config_file() {
        let (temp_dir, loader) = create_temp_config();

        // JSONC config with comments
        let config_jsonc = r#"{
            // Model configuration
            "models": {
                "main": "openai/gpt-5", // primary model
            },
            "profile": "fast", // trailing comma
        }"#;

        std::fs::write(temp_dir.path().join(CONFIG_FILE), config_jsonc).unwrap();

        let config = loader.load_config().unwrap();
        assert!(config.models.is_some());
        let models = config.models.as_ref().unwrap();
        assert_eq!(models.main.as_ref().unwrap().provider, "openai");
        assert_eq!(models.main.as_ref().unwrap().model, "gpt-5");
        assert_eq!(config.profile, Some("fast".to_string()));
    }

    #[test]
    fn test_find_config_files_sorted() {
        let (temp_dir, loader) = create_temp_config();

        // Create files in non-alphabetical order
        std::fs::write(temp_dir.path().join("z_model.json"), "[]").unwrap();
        std::fs::write(temp_dir.path().join("a_model.json"), "[]").unwrap();
        std::fs::write(temp_dir.path().join("m_model.json"), "[]").unwrap();

        let files = loader.find_config_files("model");
        assert_eq!(files.len(), 3);
        assert!(files[0].ends_with("a_model.json"));
        assert!(files[1].ends_with("m_model.json"));
        assert!(files[2].ends_with("z_model.json"));
    }

    #[test]
    fn test_find_config_files_excludes_non_matching() {
        let (temp_dir, loader) = create_temp_config();

        std::fs::write(temp_dir.path().join("model.json"), "[]").unwrap();
        std::fs::write(temp_dir.path().join("provider.json"), "[]").unwrap();
        std::fs::write(temp_dir.path().join("config.json"), "{}").unwrap();
        std::fs::write(temp_dir.path().join("other.json"), "{}").unwrap();

        let model_files = loader.find_config_files("model");
        assert_eq!(model_files.len(), 1);
        assert!(model_files[0].ends_with("model.json"));

        let provider_files = loader.find_config_files("provider");
        assert_eq!(provider_files.len(), 1);
        assert!(provider_files[0].ends_with("provider.json"));
    }
}
