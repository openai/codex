//! Reads Guardian history lifetimes and captures completed reviewer contexts for fork replay.
//! Both operate on live state. Transcript durability remains part of turn completion.

use super::session::Session;
use codex_history::CompactedItem;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::TokenCountEvent;

impl Session {
    /// Only MultiAgent V2 workers use a separate root history for review.
    pub(crate) async fn guardian_root_history_version(&self) -> Option<(ThreadId, u64)> {
        if self.multi_agent_version() != Some(MultiAgentVersion::V2) {
            return None;
        }
        let version = self.state.lock().await.history.reset_version;
        Some((self.thread_id, version))
    }

    pub(crate) async fn guardian_fork_history(&self) -> Vec<RolloutItem> {
        let state = self.state.lock().await;
        let history = &state.history;
        let window_ids = state.auto_compact_window_ids();
        let mut items = vec![RolloutItem::Compacted(Box::new(CompactedItem {
            message: String::new(),
            replacement_history: Some(history.annotated_items().to_vec()),
            guardian_history: history.guardian_history_checkpoint(),
            retained_context: Some(history.retained_context().clone()),
            mcp_resource_origins: self.services.mcp_runtime.resource_origin_checkpoint(),
            window_number: Some(state.auto_compact_window_number()),
            first_window_id: Some(window_ids.first_window_id.clone()),
            previous_window_id: window_ids.previous_window_id.clone(),
            window_id: Some(window_ids.window_id.clone()),
            compaction_response_id: None,
            latest_token_usage_record: state.latest_token_usage_record.clone(),
            resume_metadata: None,
        }))];
        if let Some(world_state) = history.world_state_checkpoint() {
            items.push(RolloutItem::WorldState(world_state));
        }
        if let Some(context) = history.reference_context_item() {
            items.push(RolloutItem::TurnContext(context));
        }
        items.push(RolloutItem::EventMsg(EventMsg::TokenCount(
            TokenCountEvent {
                info: history.token_info(),
                rate_limits: None,
            },
        )));
        items
    }
}

#[cfg(test)]
#[path = "guardian_checkpoint_tests.rs"]
mod tests;
