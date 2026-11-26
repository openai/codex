//! Source-neutral plugin catalogs, reconciliation, and source-aware enablement policy.

use crate::marketplace_policy::PluginMarketplaceFilter;
use crate::marketplace_policy::parse_configured_plugins;
use codex_config::ConfigLayerStack;
use codex_core_plugin_common::PluginIdentity;
use codex_protocol::protocol::EnvironmentPluginsConfig;
use codex_utils_path_uri::PathUri;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

/// Thread configuration shared by all installations, with executor entries scoped by environment.
#[derive(Default)]
pub struct PluginCatalogConfig<'a> {
    thread_plugin_ids: HashSet<String>,
    /// Environments that list each additional plugin ID.
    executor_plugin_ids: HashMap<&'a str, HashSet<&'a str>>,
}

impl<'a> PluginCatalogConfig<'a> {
    /// Resolve marketplace policy once and check each distinct plugin ID once per selection.
    pub fn from_thread_and_executors(
        stack: &ConfigLayerStack,
        codex_home: &Path,
        executor_plugin_configs: impl IntoIterator<Item = (&'a str, &'a EnvironmentPluginsConfig)>,
    ) -> Self {
        let mut config = stack.effective_config();
        let thread_plugins = config.get("plugins").and_then(toml::Value::as_table);
        let mut executor_plugin_ids: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (environment_id, plugins) in executor_plugin_configs {
            for id in plugins.plugins.keys() {
                // Check the original thread keys so a filtered-out ID cannot be reintroduced.
                if !thread_plugins.is_some_and(|plugins| plugins.contains_key(id)) {
                    executor_plugin_ids
                        .entry(id)
                        .or_default()
                        .insert(environment_id);
                }
            }
        }
        if thread_plugins.is_none() && executor_plugin_ids.is_empty() {
            return Self::default();
        }

        let filter = PluginMarketplaceFilter::new(&config, stack.requirements(), codex_home);
        filter.retain_plugins(&mut config);
        let Some(thread) = parse_configured_plugins(config) else {
            return Self::default();
        };
        executor_plugin_ids.retain(|plugin_id, _| filter.allows_marketplace_for(plugin_id));
        Self {
            thread_plugin_ids: thread.into_keys().collect(),
            executor_plugin_ids,
        }
    }

    pub(crate) fn includes(&self, environment_id: &str, plugin_id: &str) -> bool {
        self.thread_plugin_ids.contains(plugin_id)
            || self
                .executor_plugin_ids
                .get(plugin_id)
                .is_some_and(|environments| environments.contains(environment_id))
    }
}

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
    /// The source must enable the entry. Executor-only installations also
    /// require a named entry that passes marketplace policy.
    pub fn is_allowed_by(&self, config: &PluginCatalogConfig<'_>) -> bool {
        if !self.enabled {
            return false;
        }
        let is_cloud_plugin = self
            .locations
            .iter()
            .any(|location| matches!(location, PluginSourceLocation::Cloud { .. }));
        is_cloud_plugin
            || self.locations.iter().all(|location| match location {
                PluginSourceLocation::Executor {
                    environment_id,
                    plugin_id,
                    ..
                } => config.includes(environment_id, plugin_id),
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
        /// Opaque release ID selected by the cloud catalog for this declaration.
        release_id: Option<String>,
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
