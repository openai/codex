//! Plugin service for loading and managing plugins.
//!
//! Provides a high-level API for plugin management, including loading
//! all enabled plugins and extracting their components.

use crate::error::Result;
use crate::injection::InjectedAgent;
use crate::injection::InjectedCommand;
use crate::injection::InjectedHook;
use crate::injection::InjectedMcpServer;
use crate::injection::InjectedOutputStyle;
use crate::injection::InjectedSkill;
use crate::injection::InjectionReport;
use crate::injection::PluginInjector;
use crate::loader::LoadedPlugin;
use crate::loader::PluginLoader;
use crate::registry::PluginRegistryV2;
use crate::settings::PluginSettings;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Plugin service for loading and managing plugins.
///
/// This is the main entry point for plugin functionality. It provides:
/// - Plugin loading with settings filtering
/// - Component extraction (skills, agents, hooks, etc.)
/// - Caching of loaded plugins
pub struct PluginService {
    /// Codex home directory.
    codex_home: PathBuf,

    /// Plugin registry.
    registry: Arc<PluginRegistryV2>,

    /// Plugin settings.
    settings: Arc<PluginSettings>,

    /// Plugin loader.
    loader: Arc<PluginLoader>,

    /// Loaded plugins (cached).
    loaded_plugins: RwLock<Vec<LoadedPlugin>>,

    /// Injection report (cached).
    injection_report: RwLock<Option<InjectionReport>>,
}

impl PluginService {
    /// Create a new plugin service.
    pub async fn new(codex_home: impl Into<PathBuf>) -> Result<Self> {
        let codex_home = codex_home.into();
        debug!("Creating PluginService with home: {}", codex_home.display());

        let registry = Arc::new(PluginRegistryV2::new(&codex_home));
        let settings = Arc::new(PluginSettings::new(&codex_home));

        // Load settings from disk
        if let Err(e) = settings.load().await {
            warn!("Failed to load plugin settings: {e}");
        }

        // Load registry from disk
        if let Err(e) = registry.load().await {
            warn!("Failed to load plugin registry: {e}");
        }

        let loader = Arc::new(PluginLoader::new(registry.clone(), settings.clone()));

        Ok(Self {
            codex_home,
            registry,
            settings,
            loader,
            loaded_plugins: RwLock::new(Vec::new()),
            injection_report: RwLock::new(None),
        })
    }

    /// Get the codex home directory.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    /// Get the plugin registry.
    pub fn registry(&self) -> &Arc<PluginRegistryV2> {
        &self.registry
    }

    /// Get the plugin settings.
    pub fn settings(&self) -> &Arc<PluginSettings> {
        &self.settings
    }

    /// Get the plugin loader.
    pub fn loader(&self) -> &Arc<PluginLoader> {
        &self.loader
    }

    /// Load all enabled plugins and extract components.
    ///
    /// This loads all plugins that are:
    /// 1. Installed in the registry
    /// 2. Not disabled in settings
    ///
    /// Results are cached; subsequent calls return the cached data.
    pub async fn load_all(&self, project_path: Option<&Path>) -> Result<()> {
        // Check if already loaded
        {
            let report = self.injection_report.read().await;
            if report.is_some() {
                debug!("Plugins already loaded, using cached data");
                return Ok(());
            }
        }

        info!("Loading all enabled plugins");

        let results = self.loader.load_all_enabled(project_path).await;
        let mut loaded = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(plugin) => {
                    info!("Loaded plugin: {}", plugin.plugin_id);
                    loaded.push(plugin);
                }
                Err(e) => {
                    warn!("Failed to load plugin: {e}");
                    errors.push(e.to_string());
                }
            }
        }

        // Create injection report
        let injector = PluginInjector::new();
        let mut report = injector.inject_all(&loaded);
        report.errors.extend(errors);

        // Cache results
        {
            let mut plugins = self.loaded_plugins.write().await;
            *plugins = loaded;
        }
        {
            let mut cached_report = self.injection_report.write().await;
            *cached_report = Some(report);
        }

        Ok(())
    }

    /// Force reload all plugins, clearing cache.
    pub async fn reload(&self, project_path: Option<&Path>) -> Result<()> {
        // Clear cache
        {
            let mut report = self.injection_report.write().await;
            *report = None;
        }
        {
            let mut plugins = self.loaded_plugins.write().await;
            plugins.clear();
        }

        self.load_all(project_path).await
    }

    /// Get loaded plugins.
    pub async fn get_loaded_plugins(&self) -> Vec<LoadedPlugin> {
        self.loaded_plugins.read().await.clone()
    }

    /// Get injection report.
    pub async fn get_injection_report(&self) -> Option<InjectionReport> {
        self.injection_report.read().await.clone()
    }

    /// Get injected agents.
    pub async fn get_agents(&self) -> Vec<InjectedAgent> {
        let report = self.injection_report.read().await;
        report
            .as_ref()
            .map(|r| r.agents.clone())
            .unwrap_or_default()
    }

    /// Get injected hooks.
    pub async fn get_hooks(&self) -> Vec<InjectedHook> {
        let report = self.injection_report.read().await;
        report.as_ref().map(|r| r.hooks.clone()).unwrap_or_default()
    }

    /// Get injected skills.
    pub async fn get_skills(&self) -> Vec<InjectedSkill> {
        let report = self.injection_report.read().await;
        report
            .as_ref()
            .map(|r| r.skills.clone())
            .unwrap_or_default()
    }

    /// Get injected commands.
    pub async fn get_commands(&self) -> Vec<InjectedCommand> {
        let report = self.injection_report.read().await;
        report
            .as_ref()
            .map(|r| r.commands.clone())
            .unwrap_or_default()
    }

    /// Get injected MCP servers.
    pub async fn get_mcp_servers(&self) -> HashMap<String, InjectedMcpServer> {
        let report = self.injection_report.read().await;
        report
            .as_ref()
            .map(|r| r.mcp_servers.clone())
            .unwrap_or_default()
    }

    /// Get injected output styles.
    pub async fn get_output_styles(&self) -> Vec<InjectedOutputStyle> {
        let report = self.injection_report.read().await;
        report
            .as_ref()
            .map(|r| r.output_styles.clone())
            .unwrap_or_default()
    }

    /// Check if plugins have been loaded.
    pub async fn is_loaded(&self) -> bool {
        self.injection_report.read().await.is_some()
    }
}

impl std::fmt::Debug for PluginService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginService")
            .field("codex_home", &self.codex_home)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PLUGIN_MANIFEST_DIR;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_service() {
        let dir = tempdir().unwrap();
        let service = PluginService::new(dir.path()).await.unwrap();

        assert_eq!(service.codex_home(), dir.path());
        assert!(!service.is_loaded().await);
    }

    #[tokio::test]
    async fn test_load_empty() {
        let dir = tempdir().unwrap();
        let service = PluginService::new(dir.path()).await.unwrap();

        service.load_all(None).await.unwrap();

        assert!(service.is_loaded().await);
        assert!(service.get_agents().await.is_empty());
        assert!(service.get_hooks().await.is_empty());
    }

    #[tokio::test]
    async fn test_load_with_plugin() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins/cache/test-mp/test-plugin/1.0.0");
        let manifest_dir = plugins_dir.join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();

        // Create plugin manifest
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        // Register plugin in registry
        let service = PluginService::new(dir.path()).await.unwrap();
        let entry = crate::registry::InstallEntryV2::new(
            crate::registry::InstallScope::User,
            plugins_dir.to_string_lossy().to_string(),
        )
        .with_version("1.0.0");

        service
            .registry()
            .upsert("test-plugin@test-mp", entry)
            .await
            .unwrap();

        // Load plugins
        service.load_all(None).await.unwrap();

        let loaded = service.get_loaded_plugins().await;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].manifest.name, "test-plugin");
    }

    #[tokio::test]
    async fn test_reload() {
        let dir = tempdir().unwrap();
        let service = PluginService::new(dir.path()).await.unwrap();

        // First load
        service.load_all(None).await.unwrap();
        assert!(service.is_loaded().await);

        // Reload clears and reloads
        service.reload(None).await.unwrap();
        assert!(service.is_loaded().await);
    }
}
