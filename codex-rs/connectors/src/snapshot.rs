use std::collections::HashMap;
use std::collections::HashSet;

use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginCapabilitySummary;

/// Connector declarations contributed by one plugin package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginConnectorSource {
    plugin_id: String,
    plugin_display_name: String,
    connector_ids: Vec<AppConnectorId>,
}

impl PluginConnectorSource {
    /// Creates one plugin source from parsed app declarations.
    pub fn new(
        plugin_id: impl Into<String>,
        plugin_display_name: impl Into<String>,
        declarations: impl IntoIterator<Item = AppDeclaration>,
    ) -> Self {
        Self::from_connector_ids(
            plugin_id,
            plugin_display_name,
            declarations
                .into_iter()
                .map(|declaration| declaration.connector_id),
        )
    }

    /// Creates one plugin source from connector IDs that were already parsed.
    pub fn from_connector_ids(
        plugin_id: impl Into<String>,
        plugin_display_name: impl Into<String>,
        connector_ids: impl IntoIterator<Item = AppConnectorId>,
    ) -> Self {
        let mut seen_connector_ids = HashSet::new();
        let connector_ids = connector_ids
            .into_iter()
            .filter(|connector_id| !connector_id.0.trim().is_empty())
            .filter(|connector_id| seen_connector_ids.insert(connector_id.clone()))
            .collect();
        Self {
            plugin_id: plugin_id.into(),
            plugin_display_name: plugin_display_name.into(),
            connector_ids,
        }
    }

    /// Returns the package name shown in connector provenance.
    pub fn plugin_display_name(&self) -> &str {
        &self.plugin_display_name
    }

    /// Returns the connector IDs contributed by this package.
    pub fn connector_ids(&self) -> &[AppConnectorId] {
        &self.connector_ids
    }
}

impl From<&PluginCapabilitySummary> for PluginConnectorSource {
    fn from(summary: &PluginCapabilitySummary) -> Self {
        Self::from_connector_ids(
            summary.config_name.clone(),
            summary.display_name.clone(),
            summary.app_connector_ids.clone(),
        )
    }
}

/// Immutable connector selection and provenance after applying disabled plugins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectorSnapshot {
    connector_ids: Vec<AppConnectorId>,
    plugin_display_names_by_connector_id: HashMap<String, Vec<String>>,
    disabled_connector_ids: HashSet<String>,
}

impl ConnectorSnapshot {
    /// Builds the final selection from all plugin sources, preserving contribution order.
    /// Pass host and selected sources together so any enabled shared owner keeps a connector.
    pub fn from_plugin_sources(
        sources: impl IntoIterator<Item = PluginConnectorSource>,
        disabled_plugin_ids: &[String],
    ) -> Self {
        let mut connector_ids = Vec::new();
        let mut seen_connector_ids = HashSet::new();
        let mut plugin_display_names_by_connector_id: HashMap<String, Vec<String>> = HashMap::new();
        let mut disabled_connector_ids = HashSet::new();

        for source in sources {
            if disabled_plugin_ids.contains(&source.plugin_id) {
                disabled_connector_ids.extend(source.connector_ids.into_iter().map(|id| id.0));
                continue;
            }
            for connector_id in source.connector_ids() {
                if seen_connector_ids.insert(connector_id.0.clone()) {
                    connector_ids.push(connector_id.clone());
                }
                plugin_display_names_by_connector_id
                    .entry(connector_id.0.clone())
                    .or_default()
                    .push(source.plugin_display_name().to_string());
            }
        }
        for plugin_names in plugin_display_names_by_connector_id.values_mut() {
            plugin_names.sort_unstable();
            plugin_names.dedup();
        }
        disabled_connector_ids.retain(|id| !seen_connector_ids.contains(id));

        Self {
            connector_ids,
            plugin_display_names_by_connector_id,
            disabled_connector_ids,
        }
    }

    /// Adapts the current host plugin summaries to the connector-owned snapshot.
    pub fn from_plugin_capability_summaries(summaries: &[PluginCapabilitySummary]) -> Self {
        Self::from_plugin_sources(summaries.iter().map(PluginConnectorSource::from), &[])
    }

    /// Returns the connector IDs in source contribution order.
    pub fn connector_ids(&self) -> &[AppConnectorId] {
        &self.connector_ids
    }

    /// Connector tools excluded because no enabled plugin still contributes them.
    pub fn disabled_connector_ids(&self) -> &HashSet<String> {
        &self.disabled_connector_ids
    }

    /// Returns the package display names associated with one connector.
    pub fn plugin_display_names_for_connector_id(&self, connector_id: &str) -> &[String] {
        self.plugin_display_names_by_connector_id
            .get(connector_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
