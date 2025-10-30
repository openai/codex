//! Session-wide mutable state.

use codex_protocol::models::ResponseItem;

use crate::client_common::Prompt;
use crate::codex::SessionConfiguration;
use crate::conversation_history::ConversationHistory;
use crate::conversation_history::ResponsesApiChainState;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;

/// Persistent, session-scoped state previously stored directly on `Session`.
pub(crate) struct SessionState {
    pub(crate) session_configuration: SessionConfiguration,
    pub(crate) history: ConversationHistory,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new(session_configuration: SessionConfiguration) -> Self {
        Self {
            session_configuration,
            history: ConversationHistory::new(),
            latest_rate_limits: None,
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

    pub(crate) fn clone_history(&self) -> ConversationHistory {
        self.history.clone()
    }

    pub(crate) fn replace_history(&mut self, items: Vec<ResponseItem>) {
        self.history.replace(items);
    }

    pub(crate) fn reset_responses_api_chain(&mut self) {
        self.history.reset_responses_api_chain();
    }

    pub(crate) fn set_responses_api_chain(&mut self, chain: ResponsesApiChainState) {
        self.history.set_responses_api_chain(chain);
    }

    // Token/rate limit helpers
    pub(crate) fn update_token_info_from_usage(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<i64>,
    ) {
        self.history.update_token_info(usage, model_context_window);
    }

    pub(crate) fn token_info(&self) -> Option<TokenUsageInfo> {
        self.history.token_info()
    }

    pub(crate) fn set_rate_limits(&mut self, snapshot: RateLimitSnapshot) {
        self.latest_rate_limits = Some(snapshot);
    }

    pub(crate) fn token_info_and_rate_limits(
        &self,
    ) -> (Option<TokenUsageInfo>, Option<RateLimitSnapshot>) {
        (self.token_info(), self.latest_rate_limits.clone())
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: i64) {
        self.history.set_token_usage_full(context_window);
    }

    pub(crate) fn prompt_for_turn(&mut self, supports_responses_api_chaining: bool) -> Prompt {
        let mut prompt = Prompt::default();
        prompt.store_response = supports_responses_api_chaining;

        let prompt_items = self.history.get_history_for_prompt();
        if !supports_responses_api_chaining {
            self.reset_responses_api_chain();
            prompt.input = prompt_items;
            return prompt;
        }

        if let Some(chain_state) = self.history.responses_api_chain() {
            let previous_response_id = chain_state.last_response_id.clone();
            if let Some(last_message_id) = chain_state.last_message_id.as_ref() {
                if let Some(position) = prompt_items
                    .iter()
                    .position(|item| response_item_id(item) == Some(last_message_id))
                {
                    prompt.previous_response_id = previous_response_id;
                    prompt.input = prompt_items.iter().skip(position + 1).cloned().collect();
                    return prompt;
                }
                // Cache marker no longer present; fall back to full prompt and clear chain info.
                self.reset_responses_api_chain();
                prompt.input = prompt_items;
                return prompt;
            }

            prompt.previous_response_id = previous_response_id;
            prompt.input = prompt_items;
            return prompt;
        }

        prompt.input = prompt_items;

        prompt
    }
}

pub(crate) fn response_item_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::Reasoning { id, .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. } => Some(id.as_str()),
        _ => None,
    }
}
