use std::collections::HashMap;
use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;

use crate::metadata::connector_mention_slug;

/// Connector references discovered by a capability owner while preparing turn input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplicitConnectorMentions {
    connector_ids: HashSet<String>,
    plain_names: HashSet<String>,
}

impl ExplicitConnectorMentions {
    pub fn insert_connector_id(&mut self, connector_id: impl Into<String>) {
        self.connector_ids.insert(connector_id.into());
    }

    pub fn insert_plain_name(&mut self, name: impl Into<String>) {
        self.plain_names.insert(name.into().to_ascii_lowercase());
    }

    pub fn extend(&mut self, other: Self) {
        self.connector_ids.extend(other.connector_ids);
        self.plain_names.extend(other.plain_names);
    }

    pub fn is_empty(&self) -> bool {
        self.connector_ids.is_empty() && self.plain_names.is_empty()
    }

    pub fn resolve(&self, connectors: &[AppInfo]) -> HashSet<String> {
        let slug_counts = connectors
            .iter()
            .fold(HashMap::new(), |mut counts, connector| {
                *counts
                    .entry(connector_mention_slug(connector))
                    .or_insert(0usize) += 1;
                counts
            });
        let mut connector_ids = self.connector_ids.clone();
        connector_ids.extend(connectors.iter().filter_map(|connector| {
            let slug = connector_mention_slug(connector);
            (slug_counts.get(&slug) == Some(&1) && self.plain_names.contains(&slug))
                .then(|| connector.id.clone())
        }));
        connector_ids
    }
}

#[cfg(test)]
#[path = "explicit_tests.rs"]
mod tests;
