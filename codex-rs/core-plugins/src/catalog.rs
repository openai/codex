//! Source-neutral plugin discovery types.

use codex_utils_path_uri::PathUri;
use std::collections::BTreeMap;

/// One source's complete discovery snapshot; no entries means no plugins were found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginCatalog {
    pub entries: Vec<PluginCatalogEntry>,
    pub warnings: Vec<String>,
}

/// Manifest metadata and every location that can supply the same logical plugin.
#[derive(Clone, PartialEq, Eq)]
pub struct PluginCatalogEntry {
    pub id: PluginIdentity,
    pub display_name: String,
    pub version: Option<String>,
    /// Serialized `.mcp.json` server declarations keyed by their authored names.
    pub mcp_servers: BTreeMap<String, String>,
    pub connector_ids: Vec<String>,
    pub locations: Vec<PluginSourceLocation>,
}

impl std::fmt::Debug for PluginCatalogEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginCatalogEntry")
            .field("id", &self.id)
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

/// Stable key used to merge discoveries of the same logical plugin.
/// Identity is independent of whether the plugin is supplied by the cloud or an executor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PluginIdentity {
    /// Stable ID assigned by plugin-service and shared by its materialized installations.
    Remote { remote_plugin_id: String },
    /// Local `name@marketplace` config key for a plugin without a remote ID.
    Local { plugin_id: String },
}

impl PluginIdentity {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Remote { remote_plugin_id } => remote_plugin_id,
            Self::Local { plugin_id } => plugin_id,
        }
    }
}

/// A source that can supply a plugin; cloud URIs are opaque, not local paths.
#[derive(Clone, Debug, PartialEq, Eq)]
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
