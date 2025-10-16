//! Session-wide mutable state.

use codex_git_tooling::GhostCommit;
use codex_protocol::models::ResponseItem;

use crate::conversation_history::ConversationHistory;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;

/// Persistent, session-scoped state previously stored directly on `Session`.
#[derive(Default)]
pub(crate) struct SessionState {
    pub(crate) history: ConversationHistory,
    pub(crate) token_info: Option<TokenUsageInfo>,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
    /// Core-managed undo snapshots for `/undo` (ring buffer; bounded for memory control).
    pub(crate) undo_snapshots: Vec<GhostCommit>,
    pub(crate) undo_snapshots_disabled: bool,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new() -> Self {
        Self {
            history: ConversationHistory::new(),
            undo_snapshots: Vec::new(),
            undo_snapshots_disabled: false,
            ..Default::default()
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        self.history.record_items(items)
    }

    pub(crate) fn history_snapshot(&self) -> Vec<ResponseItem> {
        self.history.contents()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
    }

    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<u64>,
    ) {
        self.token_info = TokenUsageInfo::new_or_append(
            &self.token_info,
            &Some(usage.clone()),
            model_context_window,
        );
    }

    pub(crate) fn set_rate_limits(&mut self, snapshot: RateLimitSnapshot) {
        self.latest_rate_limits = Some(snapshot);
    }

    pub(crate) fn token_info_and_rate_limits(
        &self,
    ) -> (Option<TokenUsageInfo>, Option<RateLimitSnapshot>) {
        (self.token_info.clone(), self.latest_rate_limits.clone())
    }

    // Undo snapshot ring helpers
    pub(crate) fn push_undo_snapshot(&mut self, gc: GhostCommit) {
        const MAX_TRACKED_GHOST_COMMITS: usize = 20;
        self.undo_snapshots.push(gc);
        if self.undo_snapshots.len() > MAX_TRACKED_GHOST_COMMITS {
            self.undo_snapshots.remove(0);
        }
    }

    pub(crate) fn pop_undo_snapshot(&mut self) -> Option<GhostCommit> {
        self.undo_snapshots.pop()
    }

    pub(crate) fn push_back_undo_snapshot(&mut self, gc: GhostCommit) {
        self.undo_snapshots.push(gc);
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: u64) {
        match &mut self.token_info {
            Some(info) => info.fill_to_context_window(context_window),
            None => {
                self.token_info = Some(TokenUsageInfo::full_context_window(context_window));
            }
        }
    }

    // Pending input/approval moved to TurnState.
}
