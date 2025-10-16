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
        let rate_limits = self.latest_rate_limits.clone();
        (self.token_info.clone(), rate_limits)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::RateLimitWindow;
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    #[test]
    fn does_not_mutate_resets_based_on_elapsed_time() {
        let mut state = SessionState::new();
        let snapshot = RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 50.0,
                window_minutes: Some(300),
                resets_in_seconds: Some(120),
            }),
            secondary: None,
            captured_at: Some(Utc::now()),
        };
        state.set_rate_limits(snapshot);

        let stored = state
            .latest_rate_limits
            .as_ref()
            .expect("rate limits should be present");
        let (_, rate_limits) = state.token_info_and_rate_limits();
        let rate_limits = rate_limits.expect("rate limits should be returned");
        assert_eq!(
            rate_limits
                .primary
                .as_ref()
                .and_then(|w| w.resets_in_seconds),
            stored.primary.as_ref().and_then(|w| w.resets_in_seconds)
        );
        assert_eq!(rate_limits.captured_at, stored.captured_at);
    }
}
