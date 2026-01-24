//! Plugin manifest validation.

use crate::PLUGIN_ID_REGEX;
use crate::error::PluginError;
use crate::error::Result;
use regex::Regex;
use std::sync::LazyLock;

/// Regex for validating plugin names (kebab-case, no spaces).
static PLUGIN_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][-a-z0-9._]*$").expect("Invalid regex"));

/// Regex for validating plugin IDs (name@marketplace format).
static PLUGIN_ID_COMPILED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(PLUGIN_ID_REGEX).expect("Invalid regex"));

/// Validate a plugin name.
///
/// Plugin names must:
/// - Start with a lowercase letter or digit
/// - Contain only lowercase letters, digits, hyphens, underscores, and dots
/// - Not contain spaces
pub fn validate_plugin_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(PluginError::InvalidManifest {
            path: std::path::PathBuf::new(),
            reason: "Plugin name cannot be empty".to_string(),
        });
    }

    if name.contains(' ') {
        return Err(PluginError::InvalidManifest {
            path: std::path::PathBuf::new(),
            reason: format!(
                "Plugin name cannot contain spaces. Use kebab-case (e.g., \"my-plugin\"). Got: {name}"
            ),
        });
    }

    if !PLUGIN_NAME_REGEX.is_match(name) {
        return Err(PluginError::InvalidManifest {
            path: std::path::PathBuf::new(),
            reason: format!(
                "Invalid plugin name format: {name}. Must be kebab-case (e.g., \"my-plugin\")"
            ),
        });
    }

    Ok(())
}

/// Validate a plugin ID.
///
/// Plugin IDs must be in format: `{plugin-name}@{marketplace-name}`
/// Both parts must follow the plugin name format.
pub fn validate_plugin_id(id: &str) -> Result<()> {
    if !PLUGIN_ID_COMPILED.is_match(id) {
        return Err(PluginError::InvalidPluginId(format!(
            "Plugin ID must be in format: plugin@marketplace. Got: {id}"
        )));
    }
    Ok(())
}

/// Parse a plugin ID into (name, marketplace) parts.
pub fn parse_plugin_id(id: &str) -> Result<(&str, &str)> {
    validate_plugin_id(id)?;

    let parts: Vec<&str> = id.splitn(2, '@').collect();
    if parts.len() != 2 {
        return Err(PluginError::InvalidPluginId(format!(
            "Plugin ID must contain exactly one '@': {id}"
        )));
    }

    Ok((parts[0], parts[1]))
}

/// Create a plugin ID from name and marketplace.
pub fn make_plugin_id(name: &str, marketplace: &str) -> Result<String> {
    validate_plugin_name(name)?;
    validate_plugin_name(marketplace)?;
    Ok(format!("{name}@{marketplace}"))
}

/// Validate command metadata.
///
/// Commands must have either `source` or `content`, but not both.
pub fn validate_command_metadata(
    source: Option<&str>,
    content: Option<&str>,
    cmd_name: &str,
) -> Result<()> {
    match (source, content) {
        (Some(_), Some(_)) => Err(PluginError::InvalidManifest {
            path: std::path::PathBuf::new(),
            reason: format!(
                "Command '{cmd_name}' has both 'source' and 'content'. Only one is allowed."
            ),
        }),
        (None, None) => Err(PluginError::InvalidManifest {
            path: std::path::PathBuf::new(),
            reason: format!("Command '{cmd_name}' must have either 'source' or 'content' defined."),
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_plugin_names() {
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("plugin123").is_ok());
        assert!(validate_plugin_name("my_plugin").is_ok());
        assert!(validate_plugin_name("my.plugin").is_ok());
        assert!(validate_plugin_name("a").is_ok());
        assert!(validate_plugin_name("123").is_ok());
    }

    #[test]
    fn test_invalid_plugin_names() {
        assert!(validate_plugin_name("").is_err());
        assert!(validate_plugin_name("My Plugin").is_err());
        assert!(validate_plugin_name("my plugin").is_err());
        assert!(validate_plugin_name("MyPlugin").is_err()); // uppercase
        assert!(validate_plugin_name("-plugin").is_err()); // starts with hyphen
    }

    #[test]
    fn test_valid_plugin_ids() {
        assert!(validate_plugin_id("my-plugin@marketplace").is_ok());
        assert!(validate_plugin_id("plugin123@mp456").is_ok());
        assert!(validate_plugin_id("a@b").is_ok());
    }

    #[test]
    fn test_invalid_plugin_ids() {
        assert!(validate_plugin_id("my-plugin").is_err()); // no @
        assert!(validate_plugin_id("my-plugin@").is_err()); // empty marketplace
        assert!(validate_plugin_id("@marketplace").is_err()); // empty name
        assert!(validate_plugin_id("my plugin@marketplace").is_err()); // space
    }

    #[test]
    fn test_parse_plugin_id() {
        let (name, mp) = parse_plugin_id("my-plugin@marketplace").unwrap();
        assert_eq!(name, "my-plugin");
        assert_eq!(mp, "marketplace");
    }

    #[test]
    fn test_make_plugin_id() {
        let id = make_plugin_id("my-plugin", "marketplace").unwrap();
        assert_eq!(id, "my-plugin@marketplace");

        assert!(make_plugin_id("My Plugin", "marketplace").is_err());
    }

    #[test]
    fn test_validate_command_metadata() {
        // Valid: source only
        assert!(validate_command_metadata(Some("cmd.md"), None, "test").is_ok());

        // Valid: content only
        assert!(validate_command_metadata(None, Some("# Content"), "test").is_ok());

        // Invalid: both source and content
        assert!(validate_command_metadata(Some("cmd.md"), Some("# Content"), "test").is_err());

        // Invalid: neither source nor content
        assert!(validate_command_metadata(None, None, "test").is_err());
    }
}
