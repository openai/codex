//! Configuration file loading.
//!
//! This module handles loading configuration from JSON and TOML files in the config directory.

use crate::error::ConfigError;
use crate::toml_config::ConfigToml;
use crate::types::ActiveState;
use crate::types::ModelsFile;
use crate::types::ProfilesFile;
use crate::types::ProvidersFile;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::warn;

/// Default configuration directory path.
pub const DEFAULT_CONFIG_DIR: &str = ".cocode";

/// Configuration file names (JSON).
pub const MODELS_FILE: &str = "models.json";
pub const PROVIDERS_FILE: &str = "providers.json";
pub const PROFILES_FILE: &str = "profiles.json";
pub const ACTIVE_FILE: &str = "active.json";

/// TOML configuration file name.
pub const CONFIG_TOML_FILE: &str = "config.toml";

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
/// 2. `CLAUDE.md`
/// 3. `.cocode/AGENTS.md`
///
/// Returns `None` if no instruction file is found or if the file is empty.
pub fn load_instructions(project_dir: &Path) -> Option<String> {
    let candidates = [AGENTS_MD_FILE, "CLAUDE.md", ".cocode/AGENTS.md"];
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
            std::fs::create_dir_all(&self.config_dir).map_err(|e| {
                ConfigError::io(format!(
                    "Failed to create config directory {}: {e}",
                    self.config_dir.display(),
                ))
            })?;
            debug!(path = %self.config_dir.display(), "Created config directory");
        }
        Ok(())
    }

    /// Load the models configuration file.
    pub fn load_models(&self) -> Result<ModelsFile, ConfigError> {
        let path = self.config_dir.join(MODELS_FILE);
        self.load_json_file(&path)
    }

    /// Load the providers configuration file.
    pub fn load_providers(&self) -> Result<ProvidersFile, ConfigError> {
        let path = self.config_dir.join(PROVIDERS_FILE);
        self.load_json_file(&path)
    }

    /// Load the profiles configuration file.
    pub fn load_profiles(&self) -> Result<ProfilesFile, ConfigError> {
        let path = self.config_dir.join(PROFILES_FILE);
        self.load_json_file(&path)
    }

    /// Load the active state file.
    pub fn load_active(&self) -> Result<ActiveState, ConfigError> {
        let path = self.config_dir.join(ACTIVE_FILE);
        self.load_json_file(&path)
    }

    /// Save the active state file.
    pub fn save_active(&self, state: &ActiveState) -> Result<(), ConfigError> {
        self.ensure_dir()?;
        let path = self.config_dir.join(ACTIVE_FILE);
        self.save_json_file(&path, state)
    }

    /// Load the TOML configuration file (config.toml).
    pub fn load_config_toml(&self) -> Result<ConfigToml, ConfigError> {
        let path = self.config_dir.join(CONFIG_TOML_FILE);
        self.load_toml_file(&path)
    }

    /// Load a TOML file, returning default if it doesn't exist.
    fn load_toml_file<T: serde::de::DeserializeOwned + Default>(
        &self,
        path: &Path,
    ) -> Result<T, ConfigError> {
        if !path.exists() {
            debug!(path = %path.display(), "TOML file not found, using defaults");
            return Ok(T::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::io(format!("Failed to read {}: {e}", path.display())))?;

        // Handle empty files
        if content.trim().is_empty() {
            debug!(path = %path.display(), "TOML file is empty, using defaults");
            return Ok(T::default());
        }

        toml::from_str(&content)
            .map_err(|e| ConfigError::config(path.display().to_string(), e.to_string()))
    }

    /// Load a JSON file, returning default if it doesn't exist.
    fn load_json_file<T: serde::de::DeserializeOwned + Default>(
        &self,
        path: &Path,
    ) -> Result<T, ConfigError> {
        if !path.exists() {
            debug!(path = %path.display(), "Config file not found, using defaults");
            return Ok(T::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::io(format!("Failed to read {}: {e}", path.display())))?;

        // Handle empty files
        if content.trim().is_empty() {
            debug!(path = %path.display(), "Config file is empty, using defaults");
            return Ok(T::default());
        }

        serde_json::from_str(&content)
            .map_err(|e| ConfigError::config(path.display().to_string(), e.to_string()))
    }

    /// Save a JSON file.
    fn save_json_file<T: serde::Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), ConfigError> {
        let content = serde_json::to_string_pretty(value)
            .map_err(|e| ConfigError::config(path.display().to_string(), e.to_string()))?;

        std::fs::write(path, content)
            .map_err(|e| ConfigError::io(format!("Failed to write {}: {e}", path.display())))?;

        debug!(path = %path.display(), "Saved config file");
        Ok(())
    }

    /// Load all configuration files at once.
    pub fn load_all(&self) -> Result<LoadedConfig, ConfigError> {
        let models = self.load_models().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to load models.json, using defaults");
            ModelsFile::default()
        });

        let providers = self.load_providers().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to load providers.json, using defaults");
            ProvidersFile::default()
        });

        let profiles = self.load_profiles().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to load profiles.json, using defaults");
            ProfilesFile::default()
        });

        let active = self.load_active().unwrap_or_else(|e| {
            debug!(error = %e, "Failed to load active.json, using defaults");
            ActiveState::default()
        });

        let config_toml = self.load_config_toml().unwrap_or_else(|e| {
            debug!(error = %e, "Failed to load config.toml, using defaults");
            ConfigToml::default()
        });

        Ok(LoadedConfig {
            models,
            providers,
            profiles,
            active,
            config_toml,
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
    /// Models configuration.
    pub models: ModelsFile,
    /// Providers configuration.
    pub providers: ProvidersFile,
    /// Profiles configuration.
    pub profiles: ProfilesFile,
    /// Active state.
    pub active: ActiveState,
    /// TOML configuration.
    pub config_toml: ConfigToml,
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
    fn test_load_models_file() {
        let (temp_dir, loader) = create_temp_config();

        let models_json = r#"{
            "version": "1.0",
            "models": {
                "test-model": {
                    "display_name": "Test Model",
                    "context_window": 4096
                }
            }
        }"#;

        let models_path = temp_dir.path().join(MODELS_FILE);
        std::fs::write(&models_path, models_json).unwrap();

        let models = loader.load_models().unwrap();
        assert_eq!(models.version, "1.0");
        assert!(models.models.contains_key("test-model"));
    }

    #[test]
    fn test_load_providers_file() {
        let (temp_dir, loader) = create_temp_config();

        let providers_json = r#"{
            "version": "1.0",
            "providers": {
                "openai": {
                    "name": "OpenAI",
                    "type": "openai",
                    "env_key": "OPENAI_API_KEY"
                }
            }
        }"#;

        let providers_path = temp_dir.path().join(PROVIDERS_FILE);
        std::fs::write(&providers_path, providers_json).unwrap();

        let providers = loader.load_providers().unwrap();
        assert_eq!(providers.version, "1.0");
        assert!(providers.providers.contains_key("openai"));
    }

    #[test]
    fn test_load_profiles_file() {
        let (temp_dir, loader) = create_temp_config();

        let profiles_json = r#"{
            "version": "1.0",
            "default_profile": "coding",
            "profiles": {
                "coding": {
                    "provider": "anthropic",
                    "model": "claude-3-opus"
                }
            }
        }"#;

        let profiles_path = temp_dir.path().join(PROFILES_FILE);
        std::fs::write(&profiles_path, profiles_json).unwrap();

        let profiles = loader.load_profiles().unwrap();
        assert_eq!(profiles.default_profile, Some("coding".to_string()));
        assert!(profiles.profiles.contains_key("coding"));
    }

    #[test]
    fn test_save_and_load_active() {
        let (_temp_dir, loader) = create_temp_config();
        loader.ensure_dir().unwrap();

        let state = ActiveState {
            provider: Some("openai".to_string()),
            model: Some("gpt-4o".to_string()),
            profile: None,
            session_overrides: None,
            last_updated: None,
        };

        loader.save_active(&state).unwrap();

        let loaded = loader.load_active().unwrap();
        assert_eq!(loaded.provider, Some("openai".to_string()));
        assert_eq!(loaded.model, Some("gpt-4o".to_string()));
    }

    #[test]
    fn test_load_all() {
        let (temp_dir, loader) = create_temp_config();

        // Create minimal config files
        let models_path = temp_dir.path().join(MODELS_FILE);
        std::fs::write(&models_path, r#"{"version": "1.0", "models": {}}"#).unwrap();

        let config = loader.load_all().unwrap();
        assert_eq!(config.models.version, "1.0");
        assert!(config.providers.providers.is_empty());
    }

    #[test]
    fn test_load_empty_file() {
        let (temp_dir, loader) = create_temp_config();

        let models_path = temp_dir.path().join(MODELS_FILE);
        std::fs::write(&models_path, "").unwrap();

        let models = loader.load_models().unwrap();
        assert!(models.models.is_empty());
    }

    #[test]
    fn test_load_invalid_json() {
        let (temp_dir, loader) = create_temp_config();

        let models_path = temp_dir.path().join(MODELS_FILE);
        std::fs::write(&models_path, "{ invalid json }").unwrap();

        let result = loader.load_models();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::Config { .. }));
    }
}
