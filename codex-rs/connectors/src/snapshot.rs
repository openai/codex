use std::collections::HashMap;
use std::collections::HashSet;

use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;

/// Immutable connector declarations and their plugin provenance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectorSnapshot {
    connector_ids: Vec<AppConnectorId>,
    plugin_display_names_by_connector_id: HashMap<String, Vec<String>>,
}

impl ConnectorSnapshot {
    /// Adapts the current host plugin summaries to the connector-owned snapshot.
    pub fn from_plugin_capability_summaries(summaries: &[PluginCapabilitySummary]) -> Self {
        let mut connector_ids = Vec::new();
        let mut seen_connector_ids = HashSet::new();
        let mut plugin_display_names_by_connector_id: HashMap<String, Vec<String>> = HashMap::new();

        for summary in summaries {
            for connector_id in &summary.app_connector_ids {
                if connector_id.0.trim().is_empty() {
                    continue;
                }
                if seen_connector_ids.insert(connector_id.clone()) {
                    connector_ids.push(connector_id.clone());
                }
                plugin_display_names_by_connector_id
                    .entry(connector_id.0.clone())
                    .or_default()
                    .push(summary.display_name.clone());
            }
        }
        for plugin_names in plugin_display_names_by_connector_id.values_mut() {
            plugin_names.sort_unstable();
            plugin_names.dedup();
        }

        Self {
            connector_ids,
            plugin_display_names_by_connector_id,
        }
    }

    /// Returns the connector IDs in source contribution order.
    pub fn connector_ids(&self) -> &[AppConnectorId] {
        &self.connector_ids
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
