//! Session-wide mutable state.

use std::collections::HashMap;
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
    /// Number of most-recent turns that are pinned (immune to prune/toggle).
    pub(crate) pinned_tail_turns: usize,
    /// Registry of in-flight tool calls keyed by `call_id` to preserve
    /// referential integrity when history is pruned between turns.
    call_registry: HashMap<String, ResponseItem>,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new() -> Self {
        Self { history: ConversationHistory::new(), pinned_tail_turns: 1, ..Default::default() }
    }

    // History helpers
    pub(crate) fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        let before = self.history.contents().len();
        for item in items {
            // Record into visible history
            self.history.record_items(std::iter::once(&*item));

            // Maintain a minimal call begin registry
            match &*item {
                ResponseItem::FunctionCall { call_id, .. } => {
                    self.call_registry.insert(call_id.clone(), (*item).clone());
                }
                ResponseItem::CustomToolCall { call_id, .. } => {
                    self.call_registry.insert(call_id.clone(), (*item).clone());
                }
                ResponseItem::LocalShellCall { call_id: Some(id), .. } => {
                    self.call_registry.insert(id.clone(), (*item).clone());
                }
                ResponseItem::FunctionCallOutput { call_id, .. }
                | ResponseItem::CustomToolCallOutput { call_id, .. } => {
                    // Completed: we no longer need to keep the begin pinned
                    self.call_registry.remove(call_id);
                }
                _ => {}
            }
        }
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
    }

    pub(crate) fn included_history_snapshot(&self) -> Vec<ResponseItem> {
        let items = self.history.contents();
        if self.include_mask.is_empty() {
            return items;
        }
        let pinned_start = self
            .pinned_tail_start_index(self.pinned_tail_turns)
            .unwrap_or(usize::MAX);
        let mut out = Vec::with_capacity(items.len());
        for (i, it) in items.into_iter().enumerate() {
            if i >= pinned_start || self.include_mask.get(i).copied().unwrap_or(true) {
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
        let pinned_start = self
            .pinned_tail_start_index(self.pinned_tail_turns)
            .unwrap_or(usize::MAX);
        for &idx in indices {
            if idx >= pinned_start {
                continue;
            }
            if let Some(slot) = self.include_mask.get_mut(idx) {
                *slot = included;
            }
        }
    }

    pub(crate) fn lookup_call_begin(&self, call_id: &str) -> Option<ResponseItem> {
        self.call_registry.get(call_id).cloned()
    }

    // Turn-based accounting helpers removed in this branch; we rely on
    // fallback logic in `pinned_tail_start_index` instead.

    /// Returns the absolute index in `history` where the last `tail_turns`
    /// turns begin. If there is not enough history, returns 0. When there
    /// is no turn accounting, returns None.
    pub(crate) fn pinned_tail_start_index(&self, tail_turns: usize) -> Option<usize> {
        use crate::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
        use crate::protocol::USER_INSTRUCTIONS_OPEN_TAG;

        if tail_turns == 0 {
            return None;
        }

        if !self.turn_item_counts.is_empty() && self.turn_item_counts.len() > 1 {
            let mut total: usize = self.turn_item_counts.iter().sum();
            let mut keep: usize = 0;
            for count in self.turn_item_counts.iter().rev().take(tail_turns) {
                keep = keep.saturating_add(*count);
            }
            total = total.saturating_sub(keep);
            return Some(total);
        }

        // Fallback: find first non-system user message to consider as logical boundary.
        let mut sys_prefix_len = 0usize;
        for it in self.history.contents().iter() {
            if let ResponseItem::Message { content, .. } = it {
                let is_sys = content.iter().any(|c| match c {
                    codex_protocol::models::ContentItem::InputText { text }
                    | codex_protocol::models::ContentItem::OutputText { text } => {
                        text.starts_with(USER_INSTRUCTIONS_OPEN_TAG)
                            || text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG)
                    }
                    _ => false,
                });
                if is_sys {
                    sys_prefix_len += 1;
                    continue;
                }
            }
            break;
        }
        Some(sys_prefix_len)
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
