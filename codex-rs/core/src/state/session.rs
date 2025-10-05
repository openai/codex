//! Session-wide mutable state.

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
    /// Number of ResponseItems recorded per turn, in order.
    pub(crate) turn_item_counts: Vec<usize>,
    /// Inclusion mask for items in `history`. When empty or shorter than history,
    /// items default to included. When present, false means excluded from model input.
    pub(crate) include_mask: Vec<bool>,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new() -> Self {
        Self {
            history: ConversationHistory::new(),
            ..Default::default()
        }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        let before = self.history.contents().len();
        self.history.record_items(items);
        let after = self.history.contents().len();
        let added = after.saturating_sub(before);
        if added > 0 {
            self.include_mask.extend(std::iter::repeat_n(true, added));
        }
    }

    pub(crate) fn history_snapshot(&self) -> Vec<ResponseItem> {
        self.history.contents()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
        let len = self.history.contents().len();
        self.include_mask = std::iter::repeat_n(true, len).collect();
    }

    pub(crate) fn included_history_snapshot(&self) -> Vec<ResponseItem> {
        let items = self.history.contents();
        if self.include_mask.is_empty() {
            return items;
        }
        let mut out = Vec::with_capacity(items.len());
        for (i, it) in items.into_iter().enumerate() {
            if self.include_mask.get(i).copied().unwrap_or(true) {
                out.push(it);
            }
        }
        out
    }

    pub(crate) fn set_inclusion(&mut self, indices: &[usize], included: bool) {
        if self.include_mask.len() < self.history.contents().len() {
            let needed = self.history.contents().len() - self.include_mask.len();
            self.include_mask.extend(std::iter::repeat_n(true, needed));
        }
        for &idx in indices {
            if let Some(slot) = self.include_mask.get_mut(idx) {
                *slot = included;
            }
        }
    }

    pub(crate) fn note_turn_committed(&mut self, items_in_turn: usize) {
        self.turn_item_counts.push(items_in_turn);
    }

    /// Recompute turn_item_counts after pruning has modified the history.
    /// If the supplied new_counts is provided, replace directly.
    pub(crate) fn replace_turn_counts(&mut self, new_counts: Vec<usize>) {
        self.turn_item_counts = new_counts;
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
