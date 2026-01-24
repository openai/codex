//! Plugin manifest parsing and validation.

mod schema;
mod validation;

pub use schema::*;
pub use validation::*;

use crate::PLUGIN_MANIFEST_DIR;
use crate::PLUGIN_MANIFEST_FILE;
use crate::error::PluginError;
use crate::error::Result;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// Parse a plugin manifest from a JSON string.
pub fn parse_manifest(json: &str) -> Result<PluginManifest> {
    let manifest: PluginManifest =
        serde_json::from_str(json).map_err(|e| PluginError::InvalidManifest {
            path: PathBuf::new(),
            reason: format!("JSON parse error: {e}"),
        })?;

    // Validate required fields
    validate_plugin_name(&manifest.name)?;

    Ok(manifest)
}

/// Parse a plugin manifest from a file.
pub async fn parse_manifest_file(path: &Path) -> Result<PluginManifest> {
    let content = fs::read_to_string(path)
        .await
        .map_err(|e| PluginError::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("Failed to read file: {e}"),
        })?;

    let manifest: PluginManifest =
        serde_json::from_str(&content).map_err(|e| PluginError::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("JSON parse error: {e}"),
        })?;

    // Validate required fields
    if let Err(e) = validate_plugin_name(&manifest.name) {
        return Err(PluginError::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("Invalid plugin name: {e}"),
        });
    }

    debug!(
        "Parsed manifest for plugin '{}' from {}",
        manifest.name,
        path.display()
    );

    Ok(manifest)
}

/// Discover plugin manifest in a directory.
///
/// Looks for manifest in these locations (in order):
/// 1. `.codex-plugin/plugin.json` (Codex standard)
/// 2. `plugin.json` (legacy fallback)
///
/// Returns the path to the manifest file if found.
pub fn discover_manifest_path(dir: &Path) -> Option<PathBuf> {
    // Primary: .codex-plugin/plugin.json
    let primary = dir.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE);
    if primary.exists() {
        return Some(primary);
    }

    // Legacy: plugin.json in root
    let legacy = dir.join(PLUGIN_MANIFEST_FILE);
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

/// Load plugin manifest from a directory.
///
/// Discovers and parses the manifest file.
pub async fn load_manifest_from_dir(dir: &Path) -> Result<PluginManifest> {
    let manifest_path =
        discover_manifest_path(dir).ok_or_else(|| PluginError::InvalidManifest {
            path: dir.to_path_buf(),
            reason: format!(
                "No manifest found. Expected {} or {}",
                dir.join(PLUGIN_MANIFEST_DIR)
                    .join(PLUGIN_MANIFEST_FILE)
                    .display(),
                dir.join(PLUGIN_MANIFEST_FILE).display()
            ),
        })?;

    parse_manifest_file(&manifest_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_manifest() {
        let json = r#"{"name": "test-plugin", "version": "1.0.0"}"#;
        let manifest = parse_manifest(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_manifest_invalid_name() {
        let json = r#"{"name": "Test Plugin"}"#;
        assert!(parse_manifest(json).is_err());
    }

    #[tokio::test]
    async fn test_load_manifest_primary() {
        let dir = tempdir().unwrap();
        let manifest_dir = dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join(PLUGIN_MANIFEST_FILE),
            r#"{"name": "test-plugin"}"#,
        )
        .unwrap();

        let manifest = load_manifest_from_dir(dir.path()).await.unwrap();
        assert_eq!(manifest.name, "test-plugin");
    }

    #[tokio::test]
    async fn test_load_manifest_legacy() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(PLUGIN_MANIFEST_FILE),
            r#"{"name": "legacy-plugin"}"#,
        )
        .unwrap();

        let manifest = load_manifest_from_dir(dir.path()).await.unwrap();
        assert_eq!(manifest.name, "legacy-plugin");
    }

    #[tokio::test]
    async fn test_load_manifest_not_found() {
        let dir = tempdir().unwrap();
        assert!(load_manifest_from_dir(dir.path()).await.is_err());
    }

    #[test]
    fn test_discover_manifest_path() {
        let dir = tempdir().unwrap();

        // No manifest
        assert!(discover_manifest_path(dir.path()).is_none());

        // Legacy
        std::fs::write(dir.path().join(PLUGIN_MANIFEST_FILE), "{}").unwrap();
        assert!(discover_manifest_path(dir.path()).is_some());

        // Primary (takes precedence)
        let manifest_dir = dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(manifest_dir.join(PLUGIN_MANIFEST_FILE), "{}").unwrap();
        let path = discover_manifest_path(dir.path()).unwrap();
        assert!(path.to_string_lossy().contains(PLUGIN_MANIFEST_DIR));
    }
}
