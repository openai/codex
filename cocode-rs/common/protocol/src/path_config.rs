//! Extended path configuration.
//!
//! Defines additional path settings beyond the standard cocode_home and cwd.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Extended path configuration.
///
/// Provides additional path settings for project directory, plugin root,
/// and environment file location.
///
/// # Environment Variables
///
/// - `COCODE_PROJECT_DIR`: Override project directory (usually detected from git root)
/// - `COCODE_PLUGIN_ROOT`: Root directory for plugins/extensions
/// - `COCODE_ENV_FILE`: Path to custom .env file for loading environment variables
///
/// # Example
///
/// ```json
/// {
///   "paths": {
///     "project_dir": "/path/to/project",
///     "plugin_root": "/path/to/plugins",
///     "env_file": "/path/to/.env"
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PathConfig {
    /// Override project directory (usually detected from git root).
    #[serde(default)]
    pub project_dir: Option<PathBuf>,

    /// Root directory for plugins/extensions.
    #[serde(default)]
    pub plugin_root: Option<PathBuf>,

    /// Path to custom .env file for loading environment variables.
    #[serde(default)]
    pub env_file: Option<PathBuf>,
}

impl PathConfig {
    /// Create a new PathConfig with all paths set.
    pub fn new(
        project_dir: Option<PathBuf>,
        plugin_root: Option<PathBuf>,
        env_file: Option<PathBuf>,
    ) -> Self {
        Self {
            project_dir,
            plugin_root,
            env_file,
        }
    }

    /// Check if any paths are configured.
    pub fn is_empty(&self) -> bool {
        self.project_dir.is_none() && self.plugin_root.is_none() && self.env_file.is_none()
    }

    /// Merge another PathConfig into this one.
    ///
    /// Values from `other` override values in `self` if present.
    pub fn merge(&mut self, other: &PathConfig) {
        if other.project_dir.is_some() {
            self.project_dir = other.project_dir.clone();
        }
        if other.plugin_root.is_some() {
            self.plugin_root = other.plugin_root.clone();
        }
        if other.env_file.is_some() {
            self.env_file = other.env_file.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_config_default() {
        let config = PathConfig::default();
        assert!(config.project_dir.is_none());
        assert!(config.plugin_root.is_none());
        assert!(config.env_file.is_none());
        assert!(config.is_empty());
    }

    #[test]
    fn test_path_config_new() {
        let config = PathConfig::new(
            Some(PathBuf::from("/project")),
            Some(PathBuf::from("/plugins")),
            Some(PathBuf::from("/.env")),
        );
        assert_eq!(config.project_dir, Some(PathBuf::from("/project")));
        assert_eq!(config.plugin_root, Some(PathBuf::from("/plugins")));
        assert_eq!(config.env_file, Some(PathBuf::from("/.env")));
        assert!(!config.is_empty());
    }

    #[test]
    fn test_path_config_serde() {
        let json = r#"{
            "project_dir": "/project",
            "plugin_root": "/plugins",
            "env_file": "/.env"
        }"#;
        let config: PathConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.project_dir, Some(PathBuf::from("/project")));
        assert_eq!(config.plugin_root, Some(PathBuf::from("/plugins")));
        assert_eq!(config.env_file, Some(PathBuf::from("/.env")));
    }

    #[test]
    fn test_path_config_serde_defaults() {
        let json = r#"{}"#;
        let config: PathConfig = serde_json::from_str(json).unwrap();
        assert!(config.project_dir.is_none());
        assert!(config.plugin_root.is_none());
        assert!(config.env_file.is_none());
    }

    #[test]
    fn test_merge() {
        let mut base = PathConfig {
            project_dir: Some(PathBuf::from("/base")),
            plugin_root: Some(PathBuf::from("/base-plugins")),
            env_file: None,
        };

        let override_config = PathConfig {
            project_dir: None,
            plugin_root: Some(PathBuf::from("/override-plugins")),
            env_file: Some(PathBuf::from("/.env")),
        };

        base.merge(&override_config);

        // project_dir unchanged (override is None)
        assert_eq!(base.project_dir, Some(PathBuf::from("/base")));
        // plugin_root overridden
        assert_eq!(base.plugin_root, Some(PathBuf::from("/override-plugins")));
        // env_file added
        assert_eq!(base.env_file, Some(PathBuf::from("/.env")));
    }

    #[test]
    fn test_is_empty() {
        let config = PathConfig::default();
        assert!(config.is_empty());

        let config = PathConfig {
            project_dir: Some(PathBuf::from("/project")),
            ..Default::default()
        };
        assert!(!config.is_empty());
    }
}
