//! Thread-authorized installed-app mentions, independent from the discovery directory.

use super::*;
use crate::app_event::ConnectorsSnapshot;

impl ChatWidget {
    /// Refresh thread-authorized installed apps, coalescing pending readiness retries.
    pub(crate) fn refresh_connector_mentions(&mut self, force_refresh: bool) {
        if !self.connectors_enabled() || self.thread_id.is_none() {
            return;
        }
        if self.connectors.mention_refresh_in_flight {
            self.connectors.mention_refresh_pending =
                Some(self.connectors.mention_refresh_pending.unwrap_or_default() || force_refresh);
            return;
        }

        self.connectors.locally_disabled.clear();
        self.connectors.mention_refresh_in_flight = true;
        self.app_event_tx
            .send(AppEvent::FetchInstalledConnectorMentions {
                force_refresh,
                generation: self.connectors.generation,
            });
    }

    /// Apply authorized installed apps while preserving only newer local revocations.
    pub(crate) fn on_connector_mentions_loaded(
        &mut self,
        generation: ConnectorScopeGeneration,
        result: Result<ConnectorsSnapshot, String>,
    ) {
        if generation != self.connectors.generation {
            return;
        }

        self.connectors.mention_refresh_in_flight = false;
        self.connectors.notified_installed_app_ids = None;
        match result {
            Ok(mut snapshot) => {
                self.connectors.installed_app_ids = snapshot
                    .connectors
                    .iter()
                    .filter(|connector| connector.is_accessible)
                    .map(|connector| connector.id.clone())
                    .collect();
                snapshot.connectors.retain(|connector| {
                    connector.is_accessible
                        && connector.is_enabled
                        && !self.connectors.locally_disabled.contains(&connector.id)
                });
                self.connectors.mention_snapshot = Some(snapshot.clone());
                self.bottom_pane.set_connectors_snapshot(Some(snapshot));
            }
            Err(err) => warn!("failed to refresh installed app mentions: {err}"),
        }

        if let Some(force_refresh) = self.connectors.mention_refresh_pending.take() {
            self.refresh_connector_mentions(force_refresh);
        }
    }
}
