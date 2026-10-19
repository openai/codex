//! Source-neutral identity shared by plugin catalogs and runtime attribution.

/// Stable key used to merge discoveries of the same logical plugin.
/// Identity is independent of whether the plugin is supplied by the cloud or an executor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PluginIdentity {
    /// Stable ID assigned by plugin-service and shared by its materialized installations.
    Remote { remote_plugin_id: String },
    /// Local config key (normally `name@marketplace`), or a legacy selected-root identity.
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
