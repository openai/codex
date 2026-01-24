//! V2 plugin registry with scope-aware operations.

mod entry;
mod scope;
mod storage;

pub use entry::InstallEntryV2;
pub use scope::InstallScope;
pub use storage::InstalledPluginsV2;

use crate::error::PluginError;
use crate::error::Result;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;

/// Thread-safe plugin registry manager.
///
/// Provides scope-aware CRUD operations for plugin installations.
pub struct PluginRegistryV2 {
    /// Codex home directory (~/.codex).
    codex_home: PathBuf,

    /// In-memory registry data.
    data: Arc<RwLock<InstalledPluginsV2>>,
}

impl PluginRegistryV2 {
    /// Create a new registry manager.
    pub fn new(codex_home: impl Into<PathBuf>) -> Self {
        Self {
            codex_home: codex_home.into(),
            data: Arc::new(RwLock::new(InstalledPluginsV2::new())),
        }
    }

    /// Load registry from disk.
    pub async fn load(&self) -> Result<()> {
        let loaded = InstalledPluginsV2::load(&self.codex_home).await?;
        let mut data = self.data.write().await;
        *data = loaded;
        Ok(())
    }

    /// Save registry to disk.
    pub async fn save(&self) -> Result<()> {
        let data = self.data.read().await;
        data.save(&self.codex_home).await
    }

    /// Get all installations for a plugin.
    pub async fn get(&self, plugin_id: &str) -> Option<Vec<InstallEntryV2>> {
        let data = self.data.read().await;
        data.get(plugin_id).cloned()
    }

    /// Get installation at specific scope.
    pub async fn get_at_scope(
        &self,
        plugin_id: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> Option<InstallEntryV2> {
        let project_str = project_path.map(|p| p.to_string_lossy().to_string());
        let data = self.data.read().await;

        data.get(plugin_id)?
            .iter()
            .find(|e| e.matches(scope, project_str.as_deref()))
            .cloned()
    }

    /// Add or update an installation.
    pub async fn upsert(&self, plugin_id: &str, entry: InstallEntryV2) -> Result<()> {
        // Validate scope requirements
        if entry.scope.requires_project_path() && entry.project_path.is_none() {
            return Err(PluginError::ProjectPathRequired(entry.scope.to_string()));
        }

        let mut data = self.data.write().await;
        data.upsert(plugin_id, entry.clone());

        info!("Registered plugin {} at scope {}", plugin_id, entry.scope);

        Ok(())
    }

    /// Remove installation at scope.
    pub async fn remove(
        &self,
        plugin_id: &str,
        scope: InstallScope,
        project_path: Option<&Path>,
    ) -> Option<InstallEntryV2> {
        let project_str = project_path.map(|p| p.to_string_lossy().to_string());
        let mut data = self.data.write().await;

        let removed = data.remove(plugin_id, scope, project_str.as_deref());

        if removed.is_some() {
            info!("Removed plugin {} at scope {}", plugin_id, scope);
        }

        removed
    }

    /// List all plugins (optionally filtered by scope).
    pub async fn list(&self, scope: Option<InstallScope>) -> Vec<(String, InstallEntryV2)> {
        let data = self.data.read().await;
        let mut result = Vec::new();

        for (plugin_id, entries) in data.iter() {
            for entry in entries {
                if scope.is_none() || scope == Some(entry.scope) {
                    result.push((plugin_id.clone(), entry.clone()));
                }
            }
        }

        result
    }

    /// Resolve the effective plugin installation for a given context.
    ///
    /// Resolution priority (highest to lowest):
    /// 1. local (if project_path matches)
    /// 2. project (if project_path matches)
    /// 3. user
    /// 4. managed
    pub async fn resolve(
        &self,
        plugin_id: &str,
        project_path: Option<&Path>,
    ) -> Option<InstallEntryV2> {
        let project_str = project_path.map(|p| p.to_string_lossy().to_string());
        let data = self.data.read().await;

        let entries = data.get(plugin_id)?;

        // Try each scope in resolution order
        for scope in InstallScope::resolution_order() {
            if let Some(entry) = entries
                .iter()
                .find(|e| e.matches(*scope, project_str.as_deref()))
            {
                debug!(
                    "Resolved plugin {} to scope {} at {}",
                    plugin_id, scope, entry.install_path
                );
                return Some(entry.clone());
            }
        }

        None
    }

    /// Check if a plugin is installed at any scope.
    pub async fn is_installed(&self, plugin_id: &str) -> bool {
        let data = self.data.read().await;
        data.contains(plugin_id)
    }

    /// Get the count of unique installed plugins.
    pub async fn count(&self) -> usize {
        let data = self.data.read().await;
        data.len()
    }

    /// Get the codex home directory.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_registry_basic_operations() {
        let dir = tempdir().unwrap();
        let registry = PluginRegistryV2::new(dir.path());

        // Insert
        let entry = InstallEntryV2::new(InstallScope::User, "/path".to_string());
        registry.upsert("test@mp", entry).await.unwrap();

        // Get
        assert!(registry.is_installed("test@mp").await);
        let entries = registry.get("test@mp").await.unwrap();
        assert_eq!(entries.len(), 1);

        // Remove
        let removed = registry.remove("test@mp", InstallScope::User, None).await;
        assert!(removed.is_some());
        assert!(!registry.is_installed("test@mp").await);
    }

    #[tokio::test]
    async fn test_registry_resolution() {
        let dir = tempdir().unwrap();
        let registry = PluginRegistryV2::new(dir.path());

        // Add user-scope installation
        let user_entry =
            InstallEntryV2::new(InstallScope::User, "/user/path".to_string()).with_version("1.0.0");
        registry.upsert("test@mp", user_entry).await.unwrap();

        // Add project-scope installation
        let project_entry = InstallEntryV2::new(InstallScope::Project, "/project/path".to_string())
            .with_version("2.0.0")
            .with_project_path("/my/project");
        registry.upsert("test@mp", project_entry).await.unwrap();

        // Without project context, should resolve to user
        let resolved = registry.resolve("test@mp", None).await.unwrap();
        assert_eq!(resolved.scope, InstallScope::User);

        // With project context, should resolve to project
        let resolved = registry
            .resolve("test@mp", Some(Path::new("/my/project")))
            .await
            .unwrap();
        assert_eq!(resolved.scope, InstallScope::Project);

        // With different project, should fall back to user
        let resolved = registry
            .resolve("test@mp", Some(Path::new("/other/project")))
            .await
            .unwrap();
        assert_eq!(resolved.scope, InstallScope::User);
    }

    #[tokio::test]
    async fn test_registry_persistence() {
        let dir = tempdir().unwrap();

        // Create and populate registry
        {
            let registry = PluginRegistryV2::new(dir.path());
            let entry =
                InstallEntryV2::new(InstallScope::User, "/path".to_string()).with_version("1.0.0");
            registry.upsert("test@mp", entry).await.unwrap();
            registry.save().await.unwrap();
        }

        // Load in new registry instance
        {
            let registry = PluginRegistryV2::new(dir.path());
            registry.load().await.unwrap();
            assert!(registry.is_installed("test@mp").await);

            let entries = registry.get("test@mp").await.unwrap();
            assert_eq!(entries[0].version, Some("1.0.0".to_string()));
        }
    }

    #[tokio::test]
    async fn test_registry_scope_validation() {
        let dir = tempdir().unwrap();
        let registry = PluginRegistryV2::new(dir.path());

        // Project scope without project_path should fail
        let entry = InstallEntryV2::new(InstallScope::Project, "/path".to_string());
        let result = registry.upsert("test@mp", entry).await;
        assert!(result.is_err());

        // Project scope with project_path should succeed
        let entry = InstallEntryV2::new(InstallScope::Project, "/path".to_string())
            .with_project_path("/my/project");
        let result = registry.upsert("test@mp", entry).await;
        assert!(result.is_ok());
    }
}
