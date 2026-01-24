//! V2 registry persistence.

use super::entry::InstallEntryV2;
use crate::REGISTRY_FILENAME;
use crate::error::PluginError;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// V2 registry format.
///
/// Each plugin ID maps to an array of installations at different scopes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledPluginsV2 {
    /// Schema version (always 2).
    pub version: i32,

    /// Map of plugin ID to array of installation entries.
    #[serde(default)]
    pub plugins: HashMap<String, Vec<InstallEntryV2>>,
}

impl InstalledPluginsV2 {
    /// Create a new empty V2 registry.
    pub fn new() -> Self {
        Self {
            version: 2,
            plugins: HashMap::new(),
        }
    }

    /// Get registry file path.
    pub fn registry_path(codex_home: &Path) -> PathBuf {
        codex_home.join(REGISTRY_FILENAME)
    }

    /// Load registry from disk.
    pub async fn load(codex_home: &Path) -> Result<Self> {
        let path = Self::registry_path(codex_home);

        if !path.exists() {
            debug!(
                "Registry file does not exist at {}, creating empty",
                path.display()
            );
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&path).await.map_err(|e| {
            PluginError::Registry(format!(
                "Failed to read registry at {}: {e}",
                path.display()
            ))
        })?;

        let registry: InstalledPluginsV2 = serde_json::from_str(&content).map_err(|e| {
            PluginError::Registry(format!(
                "Failed to parse registry at {}: {e}",
                path.display()
            ))
        })?;

        if registry.version != 2 {
            return Err(PluginError::Registry(format!(
                "Unsupported registry version: {} (expected 2)",
                registry.version
            )));
        }

        debug!(
            "Loaded {} plugins from registry at {}",
            registry.plugins.len(),
            path.display()
        );

        Ok(registry)
    }

    /// Save registry to disk.
    pub async fn save(&self, codex_home: &Path) -> Result<()> {
        let path = Self::registry_path(codex_home);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                PluginError::Registry(format!(
                    "Failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PluginError::Registry(format!("Failed to serialize registry: {e}")))?;

        fs::write(&path, content).await.map_err(|e| {
            PluginError::Registry(format!(
                "Failed to write registry at {}: {e}",
                path.display()
            ))
        })?;

        debug!(
            "Saved {} plugins to registry at {}",
            self.plugins.len(),
            path.display()
        );

        Ok(())
    }

    /// Get all entries for a plugin.
    pub fn get(&self, plugin_id: &str) -> Option<&Vec<InstallEntryV2>> {
        self.plugins.get(plugin_id)
    }

    /// Get mutable entries for a plugin.
    pub fn get_mut(&mut self, plugin_id: &str) -> Option<&mut Vec<InstallEntryV2>> {
        self.plugins.get_mut(plugin_id)
    }

    /// Insert or update an entry.
    ///
    /// If an entry with the same scope and project_path exists, it is replaced.
    pub fn upsert(&mut self, plugin_id: &str, entry: InstallEntryV2) {
        let entries = self.plugins.entry(plugin_id.to_string()).or_default();

        // Find and replace existing entry with same scope/project_path
        if let Some(existing) = entries
            .iter_mut()
            .find(|e| e.matches(entry.scope, entry.project_path.as_deref()))
        {
            *existing = entry;
        } else {
            entries.push(entry);
        }
    }

    /// Remove an entry at specific scope.
    ///
    /// Returns the removed entry if found.
    pub fn remove(
        &mut self,
        plugin_id: &str,
        scope: super::scope::InstallScope,
        project_path: Option<&str>,
    ) -> Option<InstallEntryV2> {
        let entries = self.plugins.get_mut(plugin_id)?;

        let index = entries
            .iter()
            .position(|e| e.matches(scope, project_path))?;

        let removed = entries.remove(index);

        // Clean up empty arrays
        if entries.is_empty() {
            self.plugins.remove(plugin_id);
        }

        Some(removed)
    }

    /// Check if a plugin has any installations.
    pub fn contains(&self, plugin_id: &str) -> bool {
        self.plugins
            .get(plugin_id)
            .is_some_and(|entries| !entries.is_empty())
    }

    /// Get count of installed plugins (unique plugin IDs).
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Iterate over all plugins and their entries.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<InstallEntryV2>)> {
        self.plugins.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::InstallScope;

    #[test]
    fn test_registry_upsert_new() {
        let mut registry = InstalledPluginsV2::new();
        let entry = InstallEntryV2::new(InstallScope::User, "/path".to_string());

        registry.upsert("test@mp", entry.clone());

        assert!(registry.contains("test@mp"));
        assert_eq!(registry.get("test@mp").unwrap().len(), 1);
    }

    #[test]
    fn test_registry_upsert_replace() {
        let mut registry = InstalledPluginsV2::new();
        let entry1 = InstallEntryV2::new(InstallScope::User, "/path1".to_string());
        let entry2 = InstallEntryV2::new(InstallScope::User, "/path2".to_string());

        registry.upsert("test@mp", entry1);
        registry.upsert("test@mp", entry2);

        assert_eq!(registry.get("test@mp").unwrap().len(), 1);
        assert_eq!(registry.get("test@mp").unwrap()[0].install_path, "/path2");
    }

    #[test]
    fn test_registry_multi_scope() {
        let mut registry = InstalledPluginsV2::new();
        let user_entry = InstallEntryV2::new(InstallScope::User, "/user/path".to_string());
        let project_entry = InstallEntryV2::new(InstallScope::Project, "/project/path".to_string())
            .with_project_path("/my/project");

        registry.upsert("test@mp", user_entry);
        registry.upsert("test@mp", project_entry);

        assert_eq!(registry.get("test@mp").unwrap().len(), 2);
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = InstalledPluginsV2::new();
        let entry = InstallEntryV2::new(InstallScope::User, "/path".to_string());

        registry.upsert("test@mp", entry);
        let removed = registry.remove("test@mp", InstallScope::User, None);

        assert!(removed.is_some());
        assert!(!registry.contains("test@mp"));
    }

    #[test]
    fn test_registry_serialization() {
        let mut registry = InstalledPluginsV2::new();
        registry.upsert(
            "test@mp",
            InstallEntryV2::new(InstallScope::User, "/path".to_string()).with_version("1.0.0"),
        );

        let json = serde_json::to_string_pretty(&registry).unwrap();
        assert!(json.contains("\"version\": 2"));
        assert!(json.contains("\"test@mp\""));

        let parsed: InstalledPluginsV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 2);
        assert!(parsed.contains("test@mp"));
    }
}
