//! Plugin loading and component extraction.

mod cache;
mod components;
mod discovery;
mod output_styles;

pub use cache::*;
pub use components::*;
pub use discovery::*;
pub use output_styles::*;

use crate::error::Result;
use crate::manifest::PluginManifest;
use crate::registry::PluginRegistryV2;
use crate::settings::PluginSettings;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

/// Loaded plugin with extracted components.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    /// Plugin manifest.
    pub manifest: PluginManifest,

    /// Installation path.
    pub install_path: PathBuf,

    /// Plugin ID (name@marketplace).
    pub plugin_id: String,

    /// Extracted skills.
    pub skills: Vec<PluginSkill>,

    /// Extracted agents.
    pub agents: Vec<PluginAgent>,

    /// Extracted hooks.
    pub hooks: Vec<PluginHook>,

    /// Extracted MCP servers.
    pub mcp_servers: Vec<PluginMcpServer>,

    /// Extracted LSP servers.
    pub lsp_servers: Vec<PluginLspServer>,

    /// Extracted commands.
    pub commands: Vec<PluginCommand>,

    /// Extracted output styles.
    pub output_styles: Vec<PluginOutputStyle>,
}

/// Plugin loader.
pub struct PluginLoader {
    registry: Arc<PluginRegistryV2>,
    settings: Arc<PluginSettings>,
}

impl PluginLoader {
    /// Create a new plugin loader.
    pub fn new(registry: Arc<PluginRegistryV2>, settings: Arc<PluginSettings>) -> Self {
        Self { registry, settings }
    }

    /// Create a plugin loader without settings (all plugins enabled).
    pub fn new_without_settings(registry: Arc<PluginRegistryV2>) -> Self {
        // Use registry's codex_home for settings
        let settings = Arc::new(PluginSettings::new(registry.codex_home()));
        Self { registry, settings }
    }

    /// Load a plugin from a directory.
    pub async fn load_from_path(&self, path: &Path, plugin_id: &str) -> Result<LoadedPlugin> {
        let manifest = crate::manifest::load_manifest_from_dir(path).await?;
        self.load_components(manifest, path.to_path_buf(), plugin_id.to_string())
            .await
    }

    /// Load a plugin by name from registry.
    pub async fn load_by_name(
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
        self.load_from_path(&path, plugin_id).await
    }

    /// Check if a plugin is enabled.
    pub async fn is_enabled(&self, plugin_id: &str) -> bool {
        self.settings.is_enabled(plugin_id).await
    }

    /// Enable a plugin.
    pub async fn enable(&self, plugin_id: &str) -> Result<()> {
        self.settings.enable(plugin_id).await;
        self.settings.save().await
    }

    /// Disable a plugin.
    pub async fn disable(&self, plugin_id: &str) -> Result<()> {
        self.settings.disable(plugin_id).await;
        self.settings.save().await
    }

    /// Load all enabled plugins.
    ///
    /// Filters out disabled plugins based on settings.
    pub async fn load_all_enabled(&self, project_path: Option<&Path>) -> Vec<Result<LoadedPlugin>> {
        let plugins = self.registry.list(None).await;
        let mut results = Vec::new();

        // Group by plugin_id and resolve to effective installation
        let mut seen = std::collections::HashSet::new();
        for (plugin_id, _) in plugins {
            if seen.contains(&plugin_id) {
                continue;
            }
            seen.insert(plugin_id.clone());

            // Check if enabled in settings
            if !self.settings.is_enabled(&plugin_id).await {
                debug!("Skipping disabled plugin: {}", plugin_id);
                continue;
            }

            let result = self.load_by_name(&plugin_id, project_path).await;
            results.push(result);
        }

        results
    }

    /// Load components from a manifest.
    async fn load_components(
        &self,
        manifest: PluginManifest,
        install_path: PathBuf,
        plugin_id: String,
    ) -> Result<LoadedPlugin> {
        debug!(
            "Loading components for plugin '{}' from {}",
            manifest.name,
            install_path.display()
        );

        let skills = extract_skills(&manifest, &install_path).await?;
        let agents = extract_agents(&manifest, &install_path).await?;
        let hooks = extract_hooks(&manifest, &install_path).await?;
        let mcp_servers = extract_mcp_servers(&manifest, &install_path).await?;
        let lsp_servers = extract_lsp_servers(&manifest, &install_path).await?;
        let commands = extract_commands(&manifest, &install_path).await?;
        let output_styles = extract_output_styles(&manifest, &install_path).await?;

        debug!(
            "Loaded plugin '{}': {} skills, {} agents, {} hooks, {} mcp, {} lsp, {} commands, {} styles",
            manifest.name,
            skills.len(),
            agents.len(),
            hooks.len(),
            mcp_servers.len(),
            lsp_servers.len(),
            commands.len(),
            output_styles.len()
        );

        Ok(LoadedPlugin {
            manifest,
            install_path,
            plugin_id,
            skills,
            agents,
            hooks,
            mcp_servers,
            lsp_servers,
            commands,
            output_styles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PLUGIN_MANIFEST_DIR;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_from_path() {
        let dir = tempdir().unwrap();
        let manifest_dir = dir.path().join(PLUGIN_MANIFEST_DIR);
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        let registry = Arc::new(PluginRegistryV2::new(dir.path()));
        let loader = PluginLoader::new_without_settings(registry);

        let loaded = loader
            .load_from_path(dir.path(), "test-plugin@test")
            .await
            .unwrap();
        assert_eq!(loaded.manifest.name, "test-plugin");
        assert_eq!(loaded.plugin_id, "test-plugin@test");
    }
}
