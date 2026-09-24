//! Cumulative MCP result attribution, independent of the best-effort call recorder.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::mcp::McpAttribution;
use codex_protocol::mcp::McpAttributionErrorReason;
use codex_protocol::mcp::McpAttributionSource;
use codex_protocol::mcp::McpAttributionStatus;

#[derive(Clone)]
pub(super) struct McpAttributionRecorder(Arc<Mutex<State>>);

struct State {
    attribution: McpAttribution,
    revision: u64,
    persisted_revision: u64,
}

#[derive(PartialEq, Eq)]
struct SourceIdentity<'a> {
    connector_id: Option<&'a str>,
    plugin_id: Option<&'a str>,
    server_name: &'a str,
    tool_name: &'a str,
}

impl<'a> From<&'a McpAttributionSource> for SourceIdentity<'a> {
    fn from(source: &'a McpAttributionSource) -> Self {
        Self {
            connector_id: source.connector_id.as_deref(),
            plugin_id: source.plugin_id.as_deref(),
            server_name: &source.server_name,
            tool_name: &source.tool_name,
        }
    }
}

impl Default for McpAttributionRecorder {
    fn default() -> Self {
        Self::new(&InitialHistory::New)
    }
}

impl McpAttributionRecorder {
    pub(super) fn new(history: &InitialHistory) -> Self {
        let mut state = State {
            attribution: McpAttribution::default(),
            // Persist an initial checkpoint even when no MCP result has been recorded.
            revision: 1,
            persisted_revision: 0,
        };
        let mut found_checkpoint = matches!(history, InitialHistory::New | InitialHistory::Cleared);
        for item in history.get_rollout_items() {
            match item {
                RolloutItem::ResponseItem(envelope) => {
                    if let Some(metadata) = envelope.metadata.as_ref()
                        && let Some(checkpoint) = metadata.mcp_attribution.as_ref()
                    {
                        found_checkpoint = true;
                        state.merge_checkpoint(checkpoint);
                    }
                }
                RolloutItem::Compacted(compacted) => {
                    for metadata in compacted
                        .replacement_history
                        .iter()
                        .flatten()
                        .filter_map(|envelope| envelope.metadata.as_ref())
                    {
                        if let Some(checkpoint) = metadata.mcp_attribution.as_ref() {
                            found_checkpoint = true;
                            state.merge_checkpoint(checkpoint);
                        }
                    }
                }
                _ => {}
            }
        }
        if !found_checkpoint {
            // Pre-attribution history cannot establish that earlier context was MCP-free.
            state.mark_error(McpAttributionErrorReason::HistoryMissingCheckpoint);
        }
        Self(Arc::new(Mutex::new(state)))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            state.mark_error(McpAttributionErrorReason::RecorderPoisoned);
            state
        })
    }

    pub(super) fn record(&self, source: McpAttributionSource) {
        self.lock().record(source, /*restoring*/ false);
    }

    pub(super) fn snapshot(&self) -> McpAttribution {
        self.lock().attribution.clone()
    }

    pub(super) fn checkpoint(&self, force: bool) -> Option<(McpAttribution, u64)> {
        let state = self.lock();
        (force || state.revision != state.persisted_revision)
            .then(|| (state.attribution.clone(), state.revision))
    }

    pub(super) fn mark_persisted(&self, revision: u64) {
        let mut state = self.lock();
        state.persisted_revision = state.persisted_revision.max(revision.min(state.revision));
    }
}

impl State {
    fn mark_error(&mut self, reason: McpAttributionErrorReason) {
        if self.attribution.status != McpAttributionStatus::AttributionError {
            self.attribution.status = McpAttributionStatus::AttributionError;
            self.attribution.error_reason = Some(reason);
            self.revision += 1;
        }
    }

    fn merge_checkpoint(&mut self, checkpoint: &McpAttribution) {
        if checkpoint.status == McpAttributionStatus::AttributionError {
            self.mark_error(
                checkpoint
                    .error_reason
                    .unwrap_or(McpAttributionErrorReason::RestoredErrorUnknown),
            );
        } else if (checkpoint.status == McpAttributionStatus::None
            && !checkpoint.sources.is_empty())
            || (checkpoint.status == McpAttributionStatus::Complete
                && checkpoint.sources.is_empty())
        {
            self.mark_error(McpAttributionErrorReason::CheckpointInvalid);
        }
        for source in &checkpoint.sources {
            self.record(source.clone(), /*restoring*/ true);
        }
    }

    fn record(&mut self, source: McpAttributionSource, restoring: bool) {
        let identity = SourceIdentity::from(&source);
        if let Some(existing) = self
            .attribution
            .sources
            .iter()
            .find(|existing| SourceIdentity::from(*existing) == identity)
        {
            if restoring && existing.first_turn_id != source.first_turn_id {
                self.mark_error(McpAttributionErrorReason::CheckpointSourceConflict);
            }
            return;
        }
        if source.server_name.is_empty()
            || source.tool_name.is_empty()
            || source.first_turn_id.is_empty()
        {
            self.mark_error(if restoring {
                McpAttributionErrorReason::CheckpointInvalid
            } else {
                McpAttributionErrorReason::SourceInvalid
            });
            return;
        }
        if self.attribution.status == McpAttributionStatus::None {
            self.attribution.status = McpAttributionStatus::Complete;
        }
        self.attribution.sources.push(source);
        self.revision += 1;
    }
}

#[cfg(test)]
#[path = "mcp_attribution_tests.rs"]
mod tests;
