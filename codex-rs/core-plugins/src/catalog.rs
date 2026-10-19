//! Source-neutral plugin catalogs, reconciliation, and source-aware enablement policy.

use codex_config::types::PluginConfig;
use codex_core_plugin_common::PluginIdentity;
use codex_utils_path_uri::PathUri;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// One source's discovery snapshot; only a complete empty catalog establishes absence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginCatalog {
    pub entries: Vec<PluginCatalogEntry>,
    pub warnings: Vec<String>,
    /// Listing omitted entries.
    pub is_partial: bool,
}

/// Manifest metadata and every location that can supply the same logical plugin.
#[derive(Clone, PartialEq, Eq)]
pub struct PluginCatalogEntry {
    pub id: PluginIdentity,
    pub enabled: bool,
    pub display_name: String,
    pub version: String,
    /// Raw MCP server declarations keyed by their authored names.
    pub mcp_servers: BTreeMap<String, Value>,
    pub connector_ids: Vec<String>,
    pub locations: Vec<PluginSourceLocation>,
}

impl PluginCatalogEntry {
    /// Disabled entries never project. Otherwise Cloud membership supplies enablement;
    /// executor-only entries require every installation's local plugin ID in the supplied
    /// config, where omitted enablement retains the configured plugin's default.
    /// A remote identity alone does not establish Cloud membership.
    pub fn is_enabled(&self, configured_plugins: &HashMap<String, PluginConfig>) -> bool {
        if !self.enabled {
            return false;
        }
        let is_cloud_plugin = self
            .locations
            .iter()
            .any(|location| matches!(location, PluginSourceLocation::Cloud { .. }));
        is_cloud_plugin
            || self.locations.iter().all(|location| match location {
                PluginSourceLocation::Executor { plugin_id, .. } => configured_plugins
                    .get(plugin_id)
                    .is_some_and(|plugin| plugin.enabled.unwrap_or(true)),
                PluginSourceLocation::Cloud { .. } => true,
            })
    }
}

impl std::fmt::Debug for PluginCatalogEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginCatalogEntry")
            .field("id", &self.id)
            .field("enabled", &self.enabled)
            .field("display_name", &self.display_name)
            .field("version", &self.version)
            // Declaration values can contain secrets, so only log their keys.
            .field(
                "mcp_server_names",
                &self.mcp_servers.keys().collect::<Vec<_>>(),
            )
            .field("connector_ids", &self.connector_ids)
            .field("locations", &self.locations)
            .finish()
    }
}

/// A source that can supply a plugin; cloud URIs are opaque, not local paths.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PluginSourceLocation {
    Cloud {
        resource_uri: String,
        bundle_uri: Option<String>,
    },
    Executor {
        /// Executor environment that owns `root`.
        environment_id: String,
        /// Local `name@marketplace` key used for this installation in configuration.
        plugin_id: String,
        root: PathUri,
    },
}

/// Uses Cloud membership and metadata, retaining version-matched installations in order.
/// Unavailable or partial Cloud data preserves unmatched executor entries. Complete Cloud
/// catalogs disable unmatched remote entries; local entries retain their enablement.
pub fn merge_catalogs(
    executor: Option<&PluginCatalog>,
    cloud: Option<&PluginCatalog>,
) -> PluginCatalog {
    let Some(executor) = executor else {
        return cloud.cloned().unwrap_or_default();
    };
    let Some(cloud) = cloud else {
        return executor.clone();
    };
    let mut merged = cloud.clone();
    merged.is_partial |= executor.is_partial;
    merged.warnings.extend(executor.warnings.iter().cloned());
    for installation in &executor.entries {
        if matches!(&installation.id, PluginIdentity::Local { .. }) {
            merged.entries.push(installation.clone());
            continue;
        }
        // Appended executor entries are not Cloud definitions and cannot authorize each other.
        let Some(definition) = merged.entries[..cloud.entries.len()]
            .iter_mut()
            .find(|definition| definition.id == installation.id)
        else {
            let mut unmatched = installation.clone();
            if !cloud.is_partial {
                unmatched.enabled = false;
            }
            merged.entries.push(unmatched);
            continue;
        };
        if definition.version != installation.version {
            merged.warnings.push(format!(
                "plugin `{}` has different cloud and executor versions; retaining the cloud definition",
                definition.id.as_str(),
            ));
            continue;
        }
        for location in &installation.locations {
            if !definition.locations.contains(location) {
                definition.locations.push(location.clone());
            }
        }
    }
    merged
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
