//! Session-wide mutable state.

use codex_protocol::models::ResponseItem;

use crate::client_common::Prompt;
use crate::client_common::compute_full_instructions;
use crate::codex::SessionConfiguration;
use crate::conversation_history::ConversationHistory;
use crate::conversation_history::ResponsesApiChainState;
use crate::conversation_history::format_prompt_items;
use crate::model_family::ModelFamily;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::ToolsConfigParams;
use crate::tools::spec::build_specs;
use crate::tools::spec::tools_metadata_for_prompt;

/// Persistent, session-scoped state previously stored directly on `Session`.
pub(crate) struct SessionState {
    pub(crate) session_configuration: SessionConfiguration,
    pub(crate) history: ConversationHistory,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
    pub(crate) model_family: ModelFamily,
}

impl SessionState {
    /// Create a new session state mirroring previous `State::default()` semantics.
    pub(crate) fn new(
        session_configuration: SessionConfiguration,
        model_family: ModelFamily,
    ) -> Self {
        Self {
            session_configuration,
            history: ConversationHistory::new(),
            latest_rate_limits: None,
            model_family,
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

    pub(crate) fn prompt_for_turn(&mut self) -> Prompt {
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &self.model_family,
            features: &self.session_configuration.features,
        });
        let (tool_specs, _registry) = build_specs(&tools_config, None).build();
        let tool_specs = tool_specs.into_iter().map(|c| c.spec).collect::<Vec<_>>();

        let prompt_items = self.history.get_history_for_prompt();
        let chain_state = self.history.responses_api_chain();
        let (mut prompt, reset_chain) = build_prompt_from_items(prompt_items, chain_state.as_ref());
        if reset_chain {
            self.reset_responses_api_chain();
        }

        // Populate prompt fields that depend only on session state.
        let (tools_json, has_freeform_apply_patch) =
            tools_metadata_for_prompt(&tool_specs).unwrap_or_default();
        format_prompt_items(&mut prompt.input, has_freeform_apply_patch);

        let apply_patch_present = tool_specs.iter().any(|spec| spec.name() == "apply_patch");
        let base_override = self.session_configuration.base_instructions.as_deref();
        let instructions =
            compute_full_instructions(base_override, &self.model_family, apply_patch_present)
                .into_owned();

        prompt.instructions = instructions;
        prompt.tools = tools_json;
        prompt.parallel_tool_calls = self.model_family.supports_parallel_tool_calls;

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

pub(crate) fn build_prompt_from_items(
    prompt_items: Vec<ResponseItem>,
    chain_state: Option<&ResponsesApiChainState>,
) -> (Prompt, bool) {
    let mut prompt = Prompt {
        store_response: chain_state.is_some(),
        ..Prompt::default()
    };

    if let Some(state) = chain_state {
        if let Some(last_message_id) = state.last_message_id.as_ref() {
            if let Some(position) = prompt_items
                .iter()
                .position(|item| response_item_id(item) == Some(last_message_id.as_str()))
            {
                if let Some(previous_response_id) = state.last_response_id.clone() {
                    prompt.previous_response_id = Some(previous_response_id);
                }
                prompt.input = prompt_items.into_iter().skip(position + 1).collect();
                return (prompt, false);
            }
            prompt.input = prompt_items;
            return (prompt, true);
        }

        if let Some(previous_response_id) = state.last_response_id.clone() {
            prompt.previous_response_id = Some(previous_response_id);
        }
        prompt.input = prompt_items;
        return (prompt, false);
    }

    prompt.input = prompt_items;
    (prompt, false)
}
