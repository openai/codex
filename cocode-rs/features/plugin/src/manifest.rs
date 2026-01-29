//! Plugin manifest parsing.
//!
//! Each plugin contains a `PLUGIN.toml` manifest that declares its metadata
//! and contributions.

use crate::contribution::PluginContributions;
use crate::error::Result;
use crate::error::plugin_error::{InvalidManifestSnafu, IoSnafu, ManifestNotFoundSnafu};

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// The expected manifest file name.
pub const PLUGIN_TOML: &str = "PLUGIN.toml";

/// Check if a version string is valid semver format.
///
/// Validates basic MAJOR.MINOR.PATCH format with optional prerelease suffix.
/// Examples: "1.0.0", "2.3.1", "1.0.0-beta.1", "0.1.0-alpha+build"
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('-').collect();
    let version_part = parts.first().unwrap_or(&"");

    // Split on '+' to handle build metadata
    let version_part = version_part.split('+').next().unwrap_or("");

    // Must have exactly 3 numeric parts
    let nums: Vec<&str> = version_part.split('.').collect();
    if nums.len() != 3 {
        return false;
    }

    // Each part must be a valid non-negative integer
    for num in nums {
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        // No leading zeros allowed (except for "0" itself)
        if num.len() > 1 && num.starts_with('0') {
            return false;
        }
    }

    true
}

/// Plugin manifest as defined in `PLUGIN.toml`.
///
/// # Example
///
/// ```toml
/// [plugin]
/// name = "my-plugin"
/// version = "0.1.0"
/// description = "My custom plugin"
/// author = "Author Name"
///
/// [contributions]
/// skills = ["skills/"]
/// hooks = ["hooks.toml"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Core plugin metadata.
    pub plugin: PluginMetadata,

    /// Plugin contributions (skills, hooks, agents).
    #[serde(default)]
    pub contributions: PluginContributions,
}

/// Core plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin name (used as identifier).
    pub name: String,

    /// Plugin version (semver).
    pub version: String,

    /// Human-readable description.
    pub description: String,

    /// Plugin author.
    #[serde(default)]
    pub author: Option<String>,

    /// Repository URL.
    #[serde(default)]
    pub repository: Option<String>,

    /// License identifier.
    #[serde(default)]
    pub license: Option<String>,

    /// Minimum cocode version required.
    #[serde(default)]
    pub min_cocode_version: Option<String>,
}

impl PluginManifest {
    /// Load a plugin manifest from a directory.
    ///
    /// Looks for `PLUGIN.toml` in the given directory.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join(PLUGIN_TOML);

        if !manifest_path.exists() {
            return Err(ManifestNotFoundSnafu {
                path: manifest_path,
            }
            .build());
        }

        Self::from_file(&manifest_path)
    }

    /// Load a plugin manifest from a file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            IoSnafu {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
            .build()
        })?;

        Self::from_str(&content, path)
    }

    /// Parse a plugin manifest from a TOML string.
    pub fn from_str(content: &str, path: &Path) -> Result<Self> {
        toml::from_str(content).map_err(|e| {
            InvalidManifestSnafu {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
            .build()
        })
    }

    /// Validate the manifest.
    pub fn validate(&self) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Name validation
        if self.plugin.name.is_empty() {
            errors.push("Plugin name cannot be empty".to_string());
        } else if self.plugin.name.len() > 64 {
            errors.push("Plugin name too long (max 64 chars)".to_string());
        } else if !self
            .plugin
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            errors.push(
                "Plugin name can only contain alphanumeric, hyphen, or underscore".to_string(),
            );
        }

        // Version validation (semver format: MAJOR.MINOR.PATCH with optional prerelease)
        if self.plugin.version.is_empty() {
            errors.push("Plugin version cannot be empty".to_string());
        } else if !is_valid_semver(&self.plugin.version) {
            errors.push(format!(
                "Plugin version '{}' is not valid semver (expected MAJOR.MINOR.PATCH)",
                self.plugin.version
            ));
        }

        // Description validation
        if self.plugin.description.is_empty() {
            errors.push("Plugin description cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
author = "Test Author"

[contributions]
skills = ["skills/"]
hooks = ["hooks.toml"]
"#;

        let manifest = PluginManifest::from_str(toml, Path::new("test")).unwrap();
        assert_eq!(manifest.plugin.name, "test-plugin");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(manifest.plugin.author, Some("Test Author".to_string()));
        assert_eq!(manifest.contributions.skills, vec!["skills/"]);
        assert_eq!(manifest.contributions.hooks, vec!["hooks.toml"]);
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[plugin]
name = "minimal"
version = "0.1.0"
description = "Minimal plugin"
"#;

        let manifest = PluginManifest::from_str(toml, Path::new("test")).unwrap();
        assert_eq!(manifest.plugin.name, "minimal");
        assert!(manifest.contributions.skills.is_empty());
        assert!(manifest.contributions.hooks.is_empty());
    }

    #[test]
    fn test_validate_manifest() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "valid-name".to_string(),
                version: "1.0.0".to_string(),
                description: "Valid description".to_string(),
                author: None,
                repository: None,
                license: None,
                min_cocode_version: None,
            },
            contributions: PluginContributions::default(),
        };

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_name() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "".to_string(),
                version: "1.0.0".to_string(),
                description: "Description".to_string(),
                author: None,
                repository: None,
                license: None,
                min_cocode_version: None,
            },
            contributions: PluginContributions::default(),
        };

        let errors = manifest.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn test_validate_invalid_name_chars() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "invalid name!".to_string(),
                version: "1.0.0".to_string(),
                description: "Description".to_string(),
                author: None,
                repository: None,
                license: None,
                min_cocode_version: None,
            },
            contributions: PluginContributions::default(),
        };

        let errors = manifest.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("alphanumeric")));
    }
}
