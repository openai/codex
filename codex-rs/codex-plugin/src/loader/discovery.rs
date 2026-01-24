//! Plugin discovery utilities.

use crate::PLUGIN_MANIFEST_DIR;
use crate::PLUGIN_MANIFEST_FILE;
use crate::error::Result;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;
use walkdir::WalkDir;

/// Find all plugin directories in a search path.
///
/// A directory is considered a plugin if it contains:
/// - `.codex-plugin/plugin.json` (Codex standard), or
/// - `plugin.json` (legacy fallback)
pub async fn find_plugins_in_path(search_path: &Path) -> Result<Vec<PathBuf>> {
    let mut plugins = Vec::new();

    if !search_path.exists() {
        return Ok(plugins);
    }

    for entry in WalkDir::new(search_path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip the manifest directory itself (it's not a plugin, just contains the manifest)
        if path
            .file_name()
            .map(|n| n == PLUGIN_MANIFEST_DIR)
            .unwrap_or(false)
        {
            continue;
        }

        // Check for manifest
        if has_plugin_manifest(path) {
            debug!("Found plugin at {}", path.display());
            plugins.push(path.to_path_buf());
        }
    }

    Ok(plugins)
}

/// Check if a directory contains a plugin manifest.
pub fn has_plugin_manifest(dir: &Path) -> bool {
    // Primary: .codex-plugin/plugin.json
    let primary = dir.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE);
    if primary.exists() {
        return true;
    }

    // Legacy: plugin.json
    let legacy = dir.join(PLUGIN_MANIFEST_FILE);
    legacy.exists()
}

/// Get the manifest path for a plugin directory.
pub fn get_manifest_path(dir: &Path) -> Option<PathBuf> {
    let primary = dir.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE);
    if primary.exists() {
        return Some(primary);
    }

    let legacy = dir.join(PLUGIN_MANIFEST_FILE);
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

/// List SKILL.md files in a directory.
pub async fn find_skill_files(skills_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut skill_files = Vec::new();

    if !skills_dir.exists() {
        return Ok(skill_files);
    }

    let mut entries = fs::read_dir(skills_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Check if this directory contains SKILL.md
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                skill_files.push(skill_file);
            }
        }

        // Or if this is directly a SKILL.md file
        if path.is_file() && path.file_name().is_some_and(|n| n == "SKILL.md") {
            skill_files.push(path);
        }
    }

    Ok(skill_files)
}

/// List markdown files in a directory.
pub async fn find_markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !dir.exists() {
        return Ok(files);
    }

    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    files.push(path);
                }
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_find_plugins_in_path() {
        let dir = tempdir().unwrap();

        // Create a plugin with .codex-plugin directory
        let plugin_dir = dir.path().join("my-plugin").join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), "{}").unwrap();

        let plugins = find_plugins_in_path(dir.path()).await.unwrap();
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn test_has_plugin_manifest() {
        let dir = tempdir().unwrap();

        // No manifest
        assert!(!has_plugin_manifest(dir.path()));

        // Legacy manifest
        std::fs::write(dir.path().join("plugin.json"), "{}").unwrap();
        assert!(has_plugin_manifest(dir.path()));
    }

    #[tokio::test]
    async fn test_find_skill_files() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        // Create skill subdirectory
        let skill_subdir = skills_dir.join("my-skill");
        std::fs::create_dir_all(&skill_subdir).unwrap();
        std::fs::write(skill_subdir.join("SKILL.md"), "# My Skill").unwrap();

        let skills = find_skill_files(&skills_dir).await.unwrap();
        assert_eq!(skills.len(), 1);
    }
}
