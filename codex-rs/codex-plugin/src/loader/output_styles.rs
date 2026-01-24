//! Output style extraction from plugin manifests.

use crate::error::Result;
use crate::frontmatter::extract_description;
use crate::frontmatter::extract_name;
use crate::frontmatter::parse_frontmatter;
use crate::manifest::OutputStylesConfig;
use crate::manifest::PluginManifest;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// Extracted output style from a plugin.
#[derive(Debug, Clone)]
pub struct PluginOutputStyle {
    /// Style name (used for referencing).
    pub name: String,
    /// Style description.
    pub description: String,
    /// Template content (markdown or handlebars).
    pub template: String,
    /// Path to the style file.
    pub path: PathBuf,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Extract output styles from a plugin manifest.
pub async fn extract_output_styles(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginOutputStyle>> {
    let mut styles = Vec::new();

    let styles_config = match &manifest.output_styles {
        Some(config) => config,
        None => return Ok(styles),
    };

    let paths = match styles_config {
        OutputStylesConfig::Path(path) => find_style_files(&base_path.join(path)).await?,
        OutputStylesConfig::Files(files) => files.iter().map(|f| base_path.join(f)).collect(),
    };

    for style_path in paths {
        if let Some(style) = parse_style_file(&style_path, &manifest.name).await? {
            styles.push(style);
        }
    }

    Ok(styles)
}

/// Find style files in a directory (.md or .hbs).
async fn find_style_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !dir.exists() {
        return Ok(files);
    }

    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "hbs" || ext == "handlebars" {
                    files.push(path);
                }
            }
        }
    }

    Ok(files)
}

/// Parse a style file (markdown with frontmatter or handlebars template).
async fn parse_style_file(path: &Path, plugin_name: &str) -> Result<Option<PluginOutputStyle>> {
    if !path.exists() {
        debug!("Style file not found: {}", path.display());
        return Ok(None);
    }

    let content = fs::read_to_string(path).await?;

    // Extract name from filename (without extension)
    let file_name = path
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // Parse frontmatter if present (markdown files)
    let (name, description, template) = if path.extension().is_some_and(|e| e == "md") {
        let parsed = parse_frontmatter(&content);
        let name = extract_name(&parsed, &file_name);
        let description = extract_description(&parsed);
        let template = parsed.content.to_string();
        (name, description, template)
    } else {
        // Handlebars templates - use full content as template
        (file_name, String::new(), content)
    };

    Ok(Some(PluginOutputStyle {
        name,
        description,
        template,
        path: path.to_path_buf(),
        source_plugin: plugin_name.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_output_styles_path() {
        let dir = tempdir().unwrap();
        let styles_dir = dir.path().join("output-styles");
        std::fs::create_dir_all(&styles_dir).unwrap();

        // Create a markdown style file
        std::fs::write(
            styles_dir.join("compact.md"),
            r#"---
name: compact
description: Compact output format
---
# Compact Output

{{ content }}
"#,
        )
        .unwrap();

        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            output_styles: Some(OutputStylesConfig::Path("output-styles".to_string())),
            ..Default::default()
        };

        let styles = extract_output_styles(&manifest, dir.path()).await.unwrap();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].name, "compact");
        assert!(!styles[0].template.is_empty());
    }

    #[tokio::test]
    async fn test_extract_output_styles_files() {
        let dir = tempdir().unwrap();

        // Create handlebars style file directly
        std::fs::write(
            dir.path().join("verbose.hbs"),
            "{{#each items}}{{this}}{{/each}}",
        )
        .unwrap();

        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            output_styles: Some(OutputStylesConfig::Files(vec!["verbose.hbs".to_string()])),
            ..Default::default()
        };

        let styles = extract_output_styles(&manifest, dir.path()).await.unwrap();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].name, "verbose");
        assert!(styles[0].template.contains("{{#each"));
    }

    #[tokio::test]
    async fn test_find_style_files() {
        let dir = tempdir().unwrap();
        let styles_dir = dir.path().join("styles");
        std::fs::create_dir_all(&styles_dir).unwrap();

        std::fs::write(styles_dir.join("a.md"), "# A").unwrap();
        std::fs::write(styles_dir.join("b.hbs"), "B").unwrap();
        std::fs::write(styles_dir.join("c.txt"), "C").unwrap(); // Should be ignored

        let files = find_style_files(&styles_dir).await.unwrap();
        assert_eq!(files.len(), 2);
    }
}
