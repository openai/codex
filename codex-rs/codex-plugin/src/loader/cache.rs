//! Memoized plugin loader with cache management.

use crate::error::Result;
use crate::loader::LoadedPlugin;
use crate::registry::PluginRegistryV2;
use crate::settings::PluginSettings;
use dashmap::DashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

/// Cached plugin loader with memoization.
///
/// Provides caching for loaded plugins to avoid repeated filesystem access
/// and parsing. The cache can be cleared globally or per-plugin.
pub struct CachedPluginLoader {
    registry: Arc<PluginRegistryV2>,
    settings: Arc<PluginSettings>,
    cache: DashMap<String, LoadedPlugin>,
    /// Session plugin directories (from --plugin-dir).
    session_dirs: Vec<PathBuf>,
}

impl CachedPluginLoader {
    /// Create a new cached plugin loader.
    pub fn new(registry: Arc<PluginRegistryV2>, settings: Arc<PluginSettings>) -> Self {
        Self {
            registry,
            settings,
            cache: DashMap::new(),
            session_dirs: Vec::new(),
        }
    }

    /// Create a cached loader without settings (all plugins enabled).
    pub fn new_without_settings(registry: Arc<PluginRegistryV2>) -> Self {
        let settings = Arc::new(PluginSettings::new(registry.codex_home()));
        Self::new(registry, settings)
    }

    /// Add session plugin directories.
    pub fn with_session_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.session_dirs = dirs;
        self
    }

    /// Clear all cached plugins.
    pub fn clear_all(&self) {
        let count = self.cache.len();
        self.cache.clear();
        debug!("Cleared {} cached plugins", count);
    }

    /// Clear a specific plugin from cache.
    pub fn clear(&self, plugin_id: &str) {
        if self.cache.remove(plugin_id).is_some() {
            debug!("Cleared cached plugin: {}", plugin_id);
        }
    }

    /// Check if a plugin is cached.
    pub fn is_cached(&self, plugin_id: &str) -> bool {
        self.cache.contains_key(plugin_id)
    }

    /// Get cache size.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Load a plugin with caching.
    ///
    /// Returns cached version if available, otherwise loads from filesystem.
    pub async fn load_cached(
        &self,
        plugin_id: &str,
        project_path: Option<&Path>,
    ) -> Result<LoadedPlugin> {
        // Check cache first
        if let Some(cached) = self.cache.get(plugin_id) {
            debug!("Using cached plugin: {}", plugin_id);
            return Ok(cached.clone());
        }

        // Load from filesystem
        let loaded = self.load_uncached(plugin_id, project_path).await?;

        // Store in cache
        self.cache.insert(plugin_id.to_string(), loaded.clone());
        debug!("Cached plugin: {}", plugin_id);

        Ok(loaded)
    }

    /// Load a plugin without using cache (always from filesystem).
    async fn load_uncached(
        &self,
        plugin_id: &str,
        project_path: Option<&Path>,
    ) -> Result<LoadedPlugin> {
        let entry = self
            .registry
            .resolve(plugin_id, project_path)
            .await
            .ok_or_else(|| crate::error::PluginError::NotFound(plugin_id.to_string()))?;

        let path = PathBuf::from(&entry.install_path);
        let _manifest = crate::manifest::load_manifest_from_dir(&path).await?;

        // Use the internal loader logic
        let loader =
            super::PluginLoader::new(Arc::clone(&self.registry), Arc::clone(&self.settings));
        loader.load_from_path(&path, plugin_id).await
    }

    /// Reload a plugin (clear cache and load fresh).
    pub async fn reload(
        &self,
        plugin_id: &str,
        project_path: Option<&Path>,
    ) -> Result<LoadedPlugin> {
        self.clear(plugin_id);
        self.load_cached(plugin_id, project_path).await
    }

    /// Check if a plugin is enabled.
    pub async fn is_enabled(&self, plugin_id: &str) -> bool {
        self.settings.is_enabled(plugin_id).await
    }

    /// Load all enabled plugins with caching.
    pub async fn load_all_enabled(&self, project_path: Option<&Path>) -> Vec<Result<LoadedPlugin>> {
        let mut results = Vec::new();

        // Load session plugins first
        for session_dir in &self.session_dirs {
            if let Ok(plugins) = super::find_plugins_in_path(session_dir).await {
                for plugin_path in plugins {
                    let plugin_id = format!(
                        "{}@session",
                        plugin_path
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_default()
                    );

                    if !self.is_enabled(&plugin_id).await {
                        debug!("Skipping disabled session plugin: {}", plugin_id);
                        continue;
                    }

                    match self.load_from_path_cached(&plugin_path, &plugin_id).await {
                        Ok(loaded) => results.push(Ok(loaded)),
                        Err(e) => {
                            debug!("Failed to load session plugin {}: {}", plugin_id, e);
                            results.push(Err(e));
                        }
                    }
                }
            }
        }

        // Load registered plugins
        let plugins = self.registry.list(None).await;
        let mut seen = std::collections::HashSet::new();

        for (plugin_id, _) in plugins {
            if seen.contains(&plugin_id) {
                continue;
            }
            seen.insert(plugin_id.clone());

            if !self.is_enabled(&plugin_id).await {
                debug!("Skipping disabled plugin: {}", plugin_id);
                continue;
            }

            let result = self.load_cached(&plugin_id, project_path).await;
            results.push(result);
        }

        results
    }

    /// Load a plugin from path with caching.
    async fn load_from_path_cached(&self, path: &Path, plugin_id: &str) -> Result<LoadedPlugin> {
        // Check cache first
        if let Some(cached) = self.cache.get(plugin_id) {
            debug!("Using cached plugin: {}", plugin_id);
            return Ok(cached.clone());
        }

        // Load from filesystem
        let loader =
            super::PluginLoader::new(Arc::clone(&self.registry), Arc::clone(&self.settings));
        let loaded = loader.load_from_path(path, plugin_id).await?;

        // Store in cache
        self.cache.insert(plugin_id.to_string(), loaded.clone());
        debug!("Cached plugin: {}", plugin_id);

        Ok(loaded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PLUGIN_MANIFEST_DIR;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cached_loader() {
        let dir = tempdir().unwrap();
        let manifest_dir = dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        let registry = Arc::new(PluginRegistryV2::new(dir.path()));

        // Register the plugin
        let entry = crate::registry::InstallEntryV2::new(
            crate::registry::InstallScope::User,
            dir.path().to_string_lossy().to_string(),
        );
        registry.upsert("test-plugin@test", entry).await.unwrap();

        let loader = CachedPluginLoader::new_without_settings(registry);

        // First load
        let loaded = loader.load_cached("test-plugin@test", None).await.unwrap();
        assert_eq!(loaded.manifest.name, "test-plugin");
        assert!(loader.is_cached("test-plugin@test"));
        assert_eq!(loader.cache_size(), 1);

        // Second load (from cache)
        let loaded2 = loader.load_cached("test-plugin@test", None).await.unwrap();
        assert_eq!(loaded2.manifest.name, "test-plugin");

        // Clear cache
        loader.clear("test-plugin@test");
        assert!(!loader.is_cached("test-plugin@test"));
    }

    #[test]
    fn test_clear_all() {
        let dir = tempdir().unwrap();
        let registry = Arc::new(PluginRegistryV2::new(dir.path()));
        let loader = CachedPluginLoader::new_without_settings(registry);

        // Manually insert into cache for testing
        loader.cache.insert(
            "p1@test".to_string(),
            LoadedPlugin {
                manifest: crate::manifest::PluginManifest::default(),
                install_path: PathBuf::new(),
                plugin_id: "p1@test".to_string(),
                skills: vec![],
                agents: vec![],
                hooks: vec![],
                mcp_servers: vec![],
                lsp_servers: vec![],
                commands: vec![],
                output_styles: vec![],
            },
        );
        loader.cache.insert(
            "p2@test".to_string(),
            LoadedPlugin {
                manifest: crate::manifest::PluginManifest::default(),
                install_path: PathBuf::new(),
                plugin_id: "p2@test".to_string(),
                skills: vec![],
                agents: vec![],
                hooks: vec![],
                mcp_servers: vec![],
                lsp_servers: vec![],
                commands: vec![],
                output_styles: vec![],
            },
        );

        assert_eq!(loader.cache_size(), 2);
        loader.clear_all();
        assert_eq!(loader.cache_size(), 0);
    }
}
