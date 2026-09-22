//! Captures a completed Guardian model context using the existing fork replay format.
//! This snapshot is in memory only; transcript durability remains part of turn completion.

use super::session::Session;
use codex_history::CompactedItem;
use codex_history::RolloutItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenCountEvent;

impl Session {
    pub(crate) async fn guardian_fork_history(&self) -> Vec<RolloutItem> {
        let state = self.state.lock().await;
        let history = &state.history;
        let window_ids = state.auto_compact_window_ids();
        let mut items = vec![RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(history.annotated_items().to_vec()),
            guardian_history: history.guardian_history_checkpoint(),
            retained_context: Some(history.retained_context().clone()),
            mcp_resource_origins: self.services.mcp_runtime.resource_origin_checkpoint(),
            window_number: Some(state.auto_compact_window_number()),
            first_window_id: Some(window_ids.first_window_id.to_string()),
            previous_window_id: window_ids.previous_window_id.map(|id| id.to_string()),
            window_id: Some(window_ids.window_id.to_string()),
            compaction_response_id: None,
            latest_token_usage_record: state.latest_token_usage_record.clone(),
            resume_metadata: None,
        })];
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
