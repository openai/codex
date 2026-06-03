use crate::store::PluginStoreError;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::PathBuf;

const PLUGINS_DATA_DIR: &str = "plugins/data";

/// Resolves executor-readable writable roots for mutable plugin data.
pub trait PluginDataStore: Send + Sync + 'static {
    /// Returns the writable local root exposed to hooks for one plugin.
    fn plugin_data_root(&self, plugin_id: &PluginId) -> AbsolutePathBuf;
}

/// Stores mutable plugin data under the local Codex Home layout.
#[derive(Debug, Clone)]
pub struct LocalPluginDataStore {
    root: AbsolutePathBuf,
}

impl LocalPluginDataStore {
    /// Creates the local mutable plugin data store for one Codex Home.
    pub fn from_codex_home(codex_home: PathBuf) -> Result<Self, PluginStoreError> {
        let root = AbsolutePathBuf::from_absolute_path_checked(codex_home.join(PLUGINS_DATA_DIR))
            .map_err(|source| PluginStoreError::Io {
            context: "failed to resolve plugin data root",
            source,
        })?;
        Ok(Self { root })
    }
}

impl PluginDataStore for LocalPluginDataStore {
    fn plugin_data_root(&self, plugin_id: &PluginId) -> AbsolutePathBuf {
        self.root.join(format!(
            "{}-{}",
            plugin_id.plugin_name, plugin_id.marketplace_name
        ))
    }
}

#[cfg(test)]
#[path = "data_store_tests.rs"]
mod tests;
