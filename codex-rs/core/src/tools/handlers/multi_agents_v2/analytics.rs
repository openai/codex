use codex_analytics::AnalyticsEventsClient;
use codex_protocol::ThreadId;
use codex_protocol::items::CollabAgentTool;
use codex_protocol::items::CollabAgentToolCallItem;
use codex_protocol::items::CollabAgentToolCallStatus;

use crate::tools::context::ToolInvocation;
use crate::turn_timing::now_unix_timestamp_ms;

/// Records private collaborator analytics without emitting another public tool item.
pub(super) struct ToolCallAnalytics {
    client: AnalyticsEventsClient,
    turn_id: String,
    item: CollabAgentToolCallItem,
    started_at_ms: i64,
}

impl ToolCallAnalytics {
    pub(super) fn new(invocation: &ToolInvocation, tool: CollabAgentTool) -> Self {
        Self {
            client: invocation.session.services.analytics_events_client.clone(),
            turn_id: invocation.turn.sub_id.clone(),
            item: CollabAgentToolCallItem {
                // Activity and analytics must use the same ID for turn-count deduplication.
                id: invocation.call_id.clone(),
                tool,
                status: CollabAgentToolCallStatus::Interrupted,
                sender_thread_id: invocation.session.thread_id,
                receiver_thread_ids: Vec::new(),
                receiver_agents: Vec::new(),
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: Default::default(),
            },
            started_at_ms: now_unix_timestamp_ms(),
        }
    }

    pub(super) fn set_receiver(&mut self, thread_id: ThreadId) {
        self.item.receiver_thread_ids = vec![thread_id];
    }

    pub(super) fn finish<T, E>(mut self, result: &Result<T, E>) {
        self.item.status = if result.is_ok() {
            CollabAgentToolCallStatus::Completed
        } else {
            CollabAgentToolCallStatus::Failed
        };
    }
}

impl Drop for ToolCallAnalytics {
    fn drop(&mut self) {
        self.client.track_collab_tool_call(
            self.turn_id.clone(),
            self.item.clone(),
            self.started_at_ms,
            now_unix_timestamp_ms(),
        );
    }
}
