//! Plugin settings management.
//!
//! Manages plugin enable/disable state stored in settings.json.
//! Plugins are enabled by default; only disabled plugins are stored.

use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;

/// Settings filename.
const SETTINGS_FILE: &str = "settings.json";

/// Plugin settings manager.
///
/// Manages the enabled/disabled state of plugins. By default, plugins are enabled.
/// Only explicitly disabled plugins are stored in settings.json.
pub struct PluginSettings {
    /// Codex home directory (~/.codex).
    codex_home: PathBuf,

    /// In-memory settings data.
    data: Arc<RwLock<SettingsData>>,
}

/// Settings data structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsData {
    /// Map of plugin ID to enabled state.
    /// Only contains entries for disabled plugins (enabled: false).
    #[serde(default)]
    pub enabled_plugins: HashMap<String, bool>,

    /// Other settings (preserved during load/save).
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

impl PluginSettings {
    /// Create a new settings manager.
    pub fn new(codex_home: impl Into<PathBuf>) -> Self {
        Self {
            codex_home: codex_home.into(),
            data: Arc::new(RwLock::new(SettingsData::default())),
        }
    }

    /// Get the settings file path.
    fn settings_path(&self) -> PathBuf {
        self.codex_home.join(SETTINGS_FILE)
    }

    /// Load settings from disk.
    pub async fn load(&self) -> Result<()> {
        let path = self.settings_path();

        if !path.exists() {
            debug!("Settings file does not exist, using defaults");
            return Ok(());
        }

        let content = fs::read_to_string(&path).await?;
        let loaded: SettingsData = serde_json::from_str(&content)?;

        let mut data = self.data.write().await;
        *data = loaded;

        debug!(
            "Loaded plugin settings: {} disabled plugins",
            data.enabled_plugins.values().filter(|v| !**v).count()
        );

        Ok(())
    }

    /// Save settings to disk.
    pub async fn save(&self) -> Result<()> {
        let path = self.settings_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let data = self.data.read().await;
        let content = serde_json::to_string_pretty(&*data)?;
        fs::write(&path, content).await?;

        debug!("Saved plugin settings to {}", path.display());

        Ok(())
    }

    /// Check if a plugin is enabled.
    ///
    /// Returns true (enabled) by default if the plugin has no explicit setting.
    pub async fn is_enabled(&self, plugin_id: &str) -> bool {
        let data = self.data.read().await;
        // Default to enabled (true) if not in map
        data.enabled_plugins.get(plugin_id).copied().unwrap_or(true)
    }

    /// Enable a plugin.
    ///
    /// Removes the plugin from the disabled list (since enabled is default).
    pub async fn enable(&self, plugin_id: &str) {
        let mut data = self.data.write().await;
        // Remove from map (enabled is the default state)
        data.enabled_plugins.remove(plugin_id);
        info!("Enabled plugin: {}", plugin_id);
    }

    /// Disable a plugin.
    ///
    /// Adds the plugin to the disabled list.
    pub async fn disable(&self, plugin_id: &str) {
        let mut data = self.data.write().await;
        data.enabled_plugins.insert(plugin_id.to_string(), false);
        info!("Disabled plugin: {}", plugin_id);
    }

    /// Toggle a plugin's enabled state.
    ///
    /// Returns the new enabled state.
    pub async fn toggle(&self, plugin_id: &str) -> bool {
        let is_currently_enabled = self.is_enabled(plugin_id).await;
        if is_currently_enabled {
            self.disable(plugin_id).await;
            false
        } else {
            self.enable(plugin_id).await;
            true
        }
    }

    /// Get all disabled plugin IDs.
    pub async fn get_disabled(&self) -> Vec<String> {
        let data = self.data.read().await;
        data.enabled_plugins
            .iter()
            .filter(|(_, enabled)| !**enabled)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get the enabled/disabled map for all plugins that have explicit settings.
    pub async fn get_all_settings(&self) -> HashMap<String, bool> {
        let data = self.data.read().await;
        data.enabled_plugins.clone()
    }

    /// Bulk set enabled state for multiple plugins.
    pub async fn set_enabled_batch(&self, settings: HashMap<String, bool>) {
        let mut data = self.data.write().await;
        for (id, enabled) in settings {
            if enabled {
                // Remove from map (enabled is default)
                data.enabled_plugins.remove(&id);
            } else {
                data.enabled_plugins.insert(id, false);
            }
        }
    }

    /// Clear all settings (reset to defaults).
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.enabled_plugins.clear();
    }
}

/// Extension trait for filtering plugins by enabled state.
pub trait EnabledFilter {
    /// Check if a plugin is enabled.
    fn is_enabled(&self, plugin_id: &str) -> bool;
}

impl EnabledFilter for HashMap<String, bool> {
    fn is_enabled(&self, plugin_id: &str) -> bool {
        // Default to enabled if not in map
        self.get(plugin_id).copied().unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_default_enabled() {
        let dir = tempdir().unwrap();
        let settings = PluginSettings::new(dir.path());

        // Should be enabled by default
        assert!(settings.is_enabled("test@mp").await);
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let dir = tempdir().unwrap();
        let settings = PluginSettings::new(dir.path());

        // Disable
        settings.disable("test@mp").await;
        assert!(!settings.is_enabled("test@mp").await);

        // Enable
        settings.enable("test@mp").await;
        assert!(settings.is_enabled("test@mp").await);
    }

    #[tokio::test]
    async fn test_toggle() {
        let dir = tempdir().unwrap();
        let settings = PluginSettings::new(dir.path());

        // Toggle from enabled (default) to disabled
        let new_state = settings.toggle("test@mp").await;
        assert!(!new_state);
        assert!(!settings.is_enabled("test@mp").await);

        // Toggle back to enabled
        let new_state = settings.toggle("test@mp").await;
        assert!(new_state);
        assert!(settings.is_enabled("test@mp").await);
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = tempdir().unwrap();

        // Create and configure settings
        {
            let settings = PluginSettings::new(dir.path());
            settings.disable("plugin1@mp").await;
            settings.disable("plugin2@mp").await;
            settings.save().await.unwrap();
        }

        // Load in new instance
        {
            let settings = PluginSettings::new(dir.path());
            settings.load().await.unwrap();

            assert!(!settings.is_enabled("plugin1@mp").await);
            assert!(!settings.is_enabled("plugin2@mp").await);
            assert!(settings.is_enabled("plugin3@mp").await); // Not configured, default enabled
        }
    }

    #[tokio::test]
    async fn test_get_disabled() {
        let dir = tempdir().unwrap();
        let settings = PluginSettings::new(dir.path());

        settings.disable("plugin1@mp").await;
        settings.disable("plugin2@mp").await;

        let disabled = settings.get_disabled().await;
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains(&"plugin1@mp".to_string()));
        assert!(disabled.contains(&"plugin2@mp".to_string()));
    }

    #[tokio::test]
    async fn test_preserves_other_settings() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join(SETTINGS_FILE);

        // Write existing settings with other fields
        let existing = serde_json::json!({
            "enabledPlugins": { "old@mp": false },
            "theme": "dark",
            "fontSize": 14
        });
        std::fs::write(&settings_path, serde_json::to_string(&existing).unwrap()).unwrap();

        // Load and modify
        let settings = PluginSettings::new(dir.path());
        settings.load().await.unwrap();
        settings.disable("new@mp").await;
        settings.save().await.unwrap();

        // Check that other fields are preserved
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let saved: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(saved["theme"], "dark");
        assert_eq!(saved["fontSize"], 14);
        assert_eq!(saved["enabledPlugins"]["old@mp"], false);
        assert_eq!(saved["enabledPlugins"]["new@mp"], false);
    }
}
