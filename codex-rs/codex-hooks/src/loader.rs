//! JSON configuration loader for hooks.
//!
//! Loads hooks configuration from JSON files in priority order:
//! 1. Project: `.codex/hooks.json`
//! 2. User: `~/.codex/hooks.json`

use std::path::Path;
use std::path::PathBuf;

use tracing::debug;
use tracing::warn;

use crate::config::HooksJsonConfig;
use crate::error::HookError;

/// Default hooks.json filename.
pub const HOOKS_JSON_FILENAME: &str = "hooks.json";

/// Project-level hooks directory relative to cwd.
pub const PROJECT_HOOKS_DIR: &str = ".codex";

/// User-level hooks directory name.
pub const USER_HOOKS_DIR: &str = ".codex";

/// Load hooks configuration from JSON files.
///
/// Tries to load in priority order:
/// 1. Project: `{cwd}/.codex/hooks.json`
/// 2. User: `~/.codex/hooks.json`
///
/// If no config file exists, returns an empty configuration.
pub fn load_hooks_config(cwd: &Path) -> Result<HooksJsonConfig, HookError> {
    // Try project config first
    let project_path = cwd.join(PROJECT_HOOKS_DIR).join(HOOKS_JSON_FILENAME);
    if project_path.exists() {
        debug!(path = %project_path.display(), "Loading project hooks config");
        return load_from_file(&project_path);
    }

    // Fall back to user config
    if let Some(home) = dirs::home_dir() {
        let user_path = home.join(USER_HOOKS_DIR).join(HOOKS_JSON_FILENAME);
        if user_path.exists() {
            debug!(path = %user_path.display(), "Loading user hooks config");
            return load_from_file(&user_path);
        }
    }

    // No config = empty hooks (not an error)
    debug!("No hooks.json found, using empty configuration");
    Ok(HooksJsonConfig::default())
}

/// Load hooks configuration from a specific file path.
pub fn load_from_file(path: &Path) -> Result<HooksJsonConfig, HookError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        warn!(path = %path.display(), error = %e, "Failed to read hooks config file");
        HookError::ConfigError(format!("Failed to read {}: {e}", path.display()))
    })?;

    parse_hooks_json(&content, path)
}

/// Parse hooks configuration from JSON string.
fn parse_hooks_json(content: &str, path: &Path) -> Result<HooksJsonConfig, HookError> {
    serde_json::from_str(content).map_err(|e| {
        warn!(path = %path.display(), error = %e, "Failed to parse hooks config");
        HookError::ConfigError(format!("Failed to parse {}: {e}", path.display()))
    })
}

/// Get the project hooks config path.
pub fn get_project_hooks_path(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_HOOKS_DIR).join(HOOKS_JSON_FILENAME)
}

/// Get the user hooks config path.
pub fn get_user_hooks_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(USER_HOOKS_DIR).join(HOOKS_JSON_FILENAME))
}

/// Check if a hooks config file exists at either project or user level.
pub fn has_hooks_config(cwd: &Path) -> bool {
    let project_path = get_project_hooks_path(cwd);
    if project_path.exists() {
        return true;
    }

    if let Some(user_path) = get_user_hooks_path() {
        return user_path.exists();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_empty_config_when_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = load_hooks_config(temp_dir.path()).unwrap();
        assert!(!config.is_disabled());
        assert!(config.hooks.is_empty());
    }

    #[test]
    fn test_load_project_config() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let hooks_json = r#"{
            "disableAllHooks": false,
            "shellPrefix": "/test/prefix.sh",
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {"type": "command", "command": "echo test"}
                        ]
                    }
                ]
            }
        }"#;

        std::fs::write(codex_dir.join("hooks.json"), hooks_json).unwrap();

        let config = load_hooks_config(temp_dir.path()).unwrap();
        assert_eq!(config.shell_prefix, Some("/test/prefix.sh".to_string()));
        assert!(config.get_hooks("PreToolUse").is_some());
    }

    #[test]
    fn test_parse_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        std::fs::write(codex_dir.join("hooks.json"), "{ invalid json }").unwrap();

        let result = load_hooks_config(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_has_hooks_config_project() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!has_hooks_config(temp_dir.path()));

        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(codex_dir.join("hooks.json"), "{}").unwrap();

        assert!(has_hooks_config(temp_dir.path()));
    }

    #[test]
    fn test_get_project_hooks_path() {
        let path = get_project_hooks_path(Path::new("/home/user/project"));
        assert_eq!(path, PathBuf::from("/home/user/project/.codex/hooks.json"));
    }
}
