//! Marketplace schema types.

use crate::installer::PluginSource;
use crate::manifest::AuthorInfo;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Marketplace registry (stored in .marketplaces.json).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketplaceRegistry {
    /// Map of marketplace name to entry.
    #[serde(default)]
    pub entries: HashMap<String, MarketplaceEntry>,
}

/// Marketplace entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceEntry {
    /// Source for fetching the marketplace.
    pub source: MarketplaceSource,

    /// Whether this marketplace is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cached manifest (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_manifest: Option<MarketplaceManifest>,

    /// Last update timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Marketplace source for fetching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum MarketplaceSource {
    /// HTTP URL to marketplace.json.
    Url {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },

    /// GitHub repository containing marketplace.
    GitHub {
        repo: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        ref_spec: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },

    /// Git URL.
    Git {
        url: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        ref_spec: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },

    /// Local file path.
    File { path: String },

    /// Local directory containing .codex-plugin/marketplace.json.
    Directory { path: String },
}

/// Marketplace manifest (marketplace.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceManifest {
    /// Marketplace name (kebab-case).
    pub name: String,

    /// Marketplace owner/maintainer.
    pub owner: AuthorInfo,

    /// Available plugins.
    #[serde(default)]
    pub plugins: Vec<MarketplacePluginEntry>,

    /// Optional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MarketplaceMetadata>,
}

/// Plugin entry in a marketplace manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplacePluginEntry {
    /// Plugin name.
    pub name: String,

    /// Plugin source.
    pub source: MarketplacePluginSource,

    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Category (e.g., "development", "productivity").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Tags for searchability.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Require plugin.json in plugin folder (default: true).
    #[serde(default = "default_true")]
    pub strict: bool,
}

/// Plugin source within a marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarketplacePluginSource {
    /// Relative path (e.g., "./plugins/my-plugin").
    Path(String),

    /// Structured source.
    Structured(PluginSourceDef),
}

/// Structured plugin source definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum PluginSourceDef {
    /// GitHub repository.
    GitHub {
        repo: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        ref_spec: Option<String>,
    },

    /// Git URL.
    Url {
        url: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        ref_spec: Option<String>,
    },

    /// NPM package.
    Npm {
        package: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
}

/// Marketplace metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceMetadata {
    /// Base path for relative plugin sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_root: Option<String>,

    /// Marketplace version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Marketplace description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MarketplacePluginEntry {
    /// Convert to PluginSource for installation.
    pub fn to_plugin_source(&self, marketplace_root: Option<&str>) -> Option<PluginSource> {
        match &self.source {
            MarketplacePluginSource::Path(path) => {
                if let Some(root) = marketplace_root {
                    Some(PluginSource::Local {
                        path: std::path::PathBuf::from(root).join(path),
                    })
                } else {
                    Some(PluginSource::Local {
                        path: std::path::PathBuf::from(path),
                    })
                }
            }
            MarketplacePluginSource::Structured(def) => match def {
                PluginSourceDef::GitHub { repo, ref_spec } => Some(PluginSource::GitHub {
                    repo: repo.clone(),
                    ref_spec: ref_spec.clone(),
                }),
                PluginSourceDef::Url { url, ref_spec } => Some(PluginSource::Git {
                    url: url.clone(),
                    ref_spec: ref_spec.clone(),
                }),
                PluginSourceDef::Npm { .. } => None, // NPM not yet supported
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marketplace_manifest_parse() {
        let json = r#"{
            "name": "test-marketplace",
            "owner": {
                "name": "Test Owner"
            },
            "plugins": [
                {
                    "name": "test-plugin",
                    "source": "./plugins/test-plugin",
                    "description": "A test plugin"
                }
            ]
        }"#;

        let manifest: MarketplaceManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-marketplace");
        assert_eq!(manifest.plugins.len(), 1);
        assert_eq!(manifest.plugins[0].name, "test-plugin");
    }

    #[test]
    fn test_plugin_source_variants() {
        // Path variant
        let json = r#"{"name": "p", "source": "./plugins/p"}"#;
        let entry: MarketplacePluginEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry.source, MarketplacePluginSource::Path(_)));

        // GitHub variant
        let json = r#"{"name": "p", "source": {"source": "github", "repo": "owner/repo"}}"#;
        let entry: MarketplacePluginEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(
            entry.source,
            MarketplacePluginSource::Structured(PluginSourceDef::GitHub { .. })
        ));
    }

    #[test]
    fn test_to_plugin_source() {
        let entry = MarketplacePluginEntry {
            name: "test".to_string(),
            source: MarketplacePluginSource::Path("./plugins/test".to_string()),
            description: None,
            version: None,
            category: None,
            tags: vec![],
            strict: true,
        };

        let source = entry.to_plugin_source(Some("/root")).unwrap();
        assert!(matches!(source, PluginSource::Local { .. }));
    }
}
