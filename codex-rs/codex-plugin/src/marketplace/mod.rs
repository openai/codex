//! Marketplace management.

mod fetcher;
mod schema;

pub use fetcher::*;
pub use schema::*;

use crate::MARKETPLACES_FILENAME;
use crate::error::PluginError;
use crate::error::Result;
use crate::registry::PluginRegistryV2;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;

/// Marketplace manager.
pub struct MarketplaceManager {
    #[allow(dead_code)]
    registry: Arc<PluginRegistryV2>,
    marketplaces: Arc<RwLock<MarketplaceRegistry>>,
    codex_home: PathBuf,
}

impl MarketplaceManager {
    /// Create a new marketplace manager.
    pub fn new(registry: Arc<PluginRegistryV2>, codex_home: impl Into<PathBuf>) -> Self {
        Self {
            registry,
            marketplaces: Arc::new(RwLock::new(MarketplaceRegistry::default())),
            codex_home: codex_home.into(),
        }
    }

    /// Load marketplaces from disk.
    pub async fn load(&self) -> Result<()> {
        let path = self.marketplaces_path();

        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path).await?;
        let registry: MarketplaceRegistry = serde_json::from_str(&content)?;

        let mut mp = self.marketplaces.write().await;
        *mp = registry;

        debug!("Loaded {} marketplaces", mp.entries.len());

        Ok(())
    }

    /// Save marketplaces to disk.
    pub async fn save(&self) -> Result<()> {
        let path = self.marketplaces_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mp = self.marketplaces.read().await;
        let content = serde_json::to_string_pretty(&*mp)?;
        fs::write(&path, content).await?;

        debug!("Saved {} marketplaces", mp.entries.len());

        Ok(())
    }

    /// Add a marketplace.
    pub async fn add(&self, name: &str, source: MarketplaceSource) -> Result<()> {
        let mut mp = self.marketplaces.write().await;

        if mp.entries.contains_key(name) {
            return Err(PluginError::Marketplace(format!(
                "Marketplace already exists: {name}"
            )));
        }

        mp.entries.insert(
            name.to_string(),
            MarketplaceEntry {
                source,
                enabled: true,
                cached_manifest: None,
                last_updated: None,
            },
        );

        info!("Added marketplace: {name}");

        Ok(())
    }

    /// Remove a marketplace.
    pub async fn remove(&self, name: &str) -> Result<()> {
        let mut mp = self.marketplaces.write().await;

        if mp.entries.remove(name).is_none() {
            return Err(PluginError::MarketplaceNotFound(name.to_string()));
        }

        info!("Removed marketplace: {name}");

        Ok(())
    }

    /// List all marketplaces.
    pub async fn list(&self) -> Vec<(String, MarketplaceEntry)> {
        let mp = self.marketplaces.read().await;
        mp.entries
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Update a marketplace (re-fetch manifest).
    pub async fn update(&self, name: &str) -> Result<MarketplaceManifest> {
        let source = {
            let mp = self.marketplaces.read().await;
            mp.entries
                .get(name)
                .ok_or_else(|| PluginError::MarketplaceNotFound(name.to_string()))?
                .source
                .clone()
        };

        let manifest = fetch_marketplace(&source).await?;

        // Update cache
        {
            let mut mp = self.marketplaces.write().await;
            if let Some(entry) = mp.entries.get_mut(name) {
                entry.cached_manifest = Some(manifest.clone());
                entry.last_updated = Some(chrono::Utc::now().to_rfc3339());
            }
        }

        info!(
            "Updated marketplace: {name} ({} plugins)",
            manifest.plugins.len()
        );

        Ok(manifest)
    }

    /// Update all marketplaces.
    pub async fn update_all(&self) -> Vec<(String, Result<MarketplaceManifest>)> {
        let names: Vec<String> = {
            let mp = self.marketplaces.read().await;
            mp.entries.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for name in names {
            let result = self.update(&name).await;
            results.push((name, result));
        }

        results
    }

    /// Get marketplace manifest (from cache or fetch).
    pub async fn get_manifest(&self, name: &str) -> Result<MarketplaceManifest> {
        // Try cache first
        {
            let mp = self.marketplaces.read().await;
            if let Some(entry) = mp.entries.get(name) {
                if let Some(ref manifest) = entry.cached_manifest {
                    return Ok(manifest.clone());
                }
            }
        }

        // Fetch if not cached
        self.update(name).await
    }

    /// Find a plugin in all marketplaces.
    pub async fn find_plugin(&self, plugin_name: &str) -> Option<(String, MarketplacePluginEntry)> {
        let names: Vec<String> = {
            let mp = self.marketplaces.read().await;
            mp.entries.keys().cloned().collect()
        };

        for name in names {
            if let Ok(manifest) = self.get_manifest(&name).await {
                if let Some(plugin) = manifest.plugins.iter().find(|p| p.name == plugin_name) {
                    return Some((name, plugin.clone()));
                }
            }
        }

        None
    }

    /// Search plugins across all marketplaces.
    pub async fn search(&self, query: &str) -> Vec<(String, MarketplacePluginEntry)> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        let names: Vec<String> = {
            let mp = self.marketplaces.read().await;
            mp.entries.keys().cloned().collect()
        };

        for name in names {
            if let Ok(manifest) = self.get_manifest(&name).await {
                for plugin in manifest.plugins {
                    let matches = plugin.name.to_lowercase().contains(&query_lower)
                        || plugin
                            .description
                            .as_ref()
                            .is_some_and(|d| d.to_lowercase().contains(&query_lower))
                        || plugin
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&query_lower));

                    if matches {
                        results.push((name.clone(), plugin));
                    }
                }
            }
        }

        results
    }

    /// Get the marketplaces config file path.
    fn marketplaces_path(&self) -> PathBuf {
        self.codex_home.join("plugins").join(MARKETPLACES_FILENAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_add_remove_marketplace() {
        let dir = tempdir().unwrap();
        let registry = Arc::new(PluginRegistryV2::new(dir.path()));
        let manager = MarketplaceManager::new(registry, dir.path());

        // Add
        let source = MarketplaceSource::Url {
            url: "https://example.com/marketplace.json".to_string(),
            headers: HashMap::new(),
        };
        manager.add("test-mp", source).await.unwrap();

        let list = manager.list().await;
        assert_eq!(list.len(), 1);

        // Remove
        manager.remove("test-mp").await.unwrap();
        let list = manager.list().await;
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_marketplace_persistence() {
        let dir = tempdir().unwrap();

        // Create and save
        {
            let registry = Arc::new(PluginRegistryV2::new(dir.path()));
            let manager = MarketplaceManager::new(registry, dir.path());

            let source = MarketplaceSource::GitHub {
                repo: "test/repo".to_string(),
                ref_spec: None,
                path: None,
            };
            manager.add("test-mp", source).await.unwrap();
            manager.save().await.unwrap();
        }

        // Load in new instance
        {
            let registry = Arc::new(PluginRegistryV2::new(dir.path()));
            let manager = MarketplaceManager::new(registry, dir.path());
            manager.load().await.unwrap();

            let list = manager.list().await;
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].0, "test-mp");
        }
    }
}
