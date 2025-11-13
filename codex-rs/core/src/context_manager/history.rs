use crate::codex::TurnContext;
use crate::context_manager::normalize;
use crate::context_manager::truncate::format_output_for_model_body;
use crate::context_manager::truncate::globally_truncate_function_output_items;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_utils_tokenizer::Tokenizer;
use std::ops::Deref;

/// Transcript of conversation history
#[derive(Debug, Clone, Default)]
pub(crate) struct ContextManager {
    /// The oldest items are at the beginning of the vector.
    items: Vec<ResponseItem>,
    token_info: Option<TokenUsageInfo>,
}

impl ContextManager {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            token_info: TokenUsageInfo::new_or_append(&None, &None, None),
        }
    }

    pub(crate) fn token_info(&self) -> Option<TokenUsageInfo> {
        self.token_info.clone()
    }

    pub(crate) fn set_token_info(&mut self, info: Option<TokenUsageInfo>) {
        self.token_info = info;
    }

    pub(crate) fn set_token_usage_full(&mut self, context_window: i64) {
        match &mut self.token_info {
            Some(info) => info.fill_to_context_window(context_window),
            None => {
                self.token_info = Some(TokenUsageInfo::full_context_window(context_window));
            }
        }
    }

    /// `items` is ordered from oldest to newest.
    pub(crate) fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator,
        I::Item: std::ops::Deref<Target = ResponseItem>,
    {
        for item in items {
            let item_ref = item.deref();
            let is_ghost_snapshot = matches!(item_ref, ResponseItem::GhostSnapshot { .. });
            if !is_api_message(item_ref) && !is_ghost_snapshot {
                continue;
            }

            let processed = Self::process_item(&item);
            self.items.push(processed);
        }
    }

    pub(crate) fn get_history(&mut self) -> Vec<ResponseItem> {
        self.normalize_history();
        self.contents()
    }

    // Returns the history prepared for sending to the model.
    // With extra response items filtered out and GhostCommits removed.
    pub(crate) fn get_history_for_prompt(&mut self) -> Vec<ResponseItem> {
        let mut history = self.get_history();
        Self::remove_ghost_snapshots(&mut history);
        history
    }

    // Estimate the number of tokens in the history. Return None if no tokenizer
    // is available. This does not consider the reasoning traces.
    // /!\ The value is a lower bound estimate and does not represent the exact
    // context length.
    pub(crate) fn estimate_token_count(&self, turn_context: &TurnContext) -> Option<i64> {
        let model = turn_context.client.get_model();
        let tokenizer = Tokenizer::for_model(model.as_str()).ok()?;
        let model_family = turn_context.client.get_model_family();

        Some(
            self.items
                .iter()
                .map(|item| {
                    serde_json::to_string(&item)
                        .map(|item| tokenizer.count(&item))
                        .unwrap_or_default()
                })
                .sum::<i64>()
                + tokenizer.count(model_family.base_instructions.as_str()),
        )
    }

    pub(crate) fn remove_first_item(&mut self) {
        if !self.items.is_empty() {
            // Remove the oldest item (front of the list). Items are ordered from
            // oldest → newest, so index 0 is the first entry recorded.
            let removed = self.items.remove(0);
            // If the removed item participates in a call/output pair, also remove
            // its corresponding counterpart to keep the invariants intact without
            // running a full normalization pass.
            normalize::remove_corresponding_for(&mut self.items, &removed);
        }
    }

    pub(crate) fn replace(&mut self, items: Vec<ResponseItem>) {
        self.items = items;
    }

    pub(crate) fn update_token_info(
        &mut self,
        usage: &TokenUsage,
        model_context_window: Option<i64>,
    ) {
        self.token_info = TokenUsageInfo::new_or_append(
            &self.token_info,
            &Some(usage.clone()),
            model_context_window,
        );
    }

    /// This function enforces a couple of invariants on the in-memory history:
    /// 1. every call (function/custom) has a corresponding output entry
    /// 2. every output has a corresponding call entry
    fn normalize_history(&mut self) {
        // all function/tool calls must have a corresponding output
        normalize::ensure_call_outputs_present(&mut self.items);

        // all outputs must have a corresponding function/tool call
        normalize::remove_orphan_outputs(&mut self.items);
    }

    /// Returns a clone of the contents in the transcript.
    fn contents(&self) -> Vec<ResponseItem> {
        self.items.clone()
    }

    fn remove_ghost_snapshots(items: &mut Vec<ResponseItem>) {
        items.retain(|item| !matches!(item, ResponseItem::GhostSnapshot { .. }));
    }

    fn process_item(item: &ResponseItem) -> ResponseItem {
        match item {
            ResponseItem::FunctionCallOutput { call_id, output } => {
                let truncated = format_output_for_model_body(output.content.as_str());
                let truncated_items = output
                    .content_items
                    .as_ref()
                    .map(|items| globally_truncate_function_output_items(items));
                ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        content: truncated,
                        content_items: truncated_items,
                        success: output.success,
                    },
                }
            }
            ResponseItem::CustomToolCallOutput { call_id, output } => {
                let truncated = format_output_for_model_body(output);
                ResponseItem::CustomToolCallOutput {
                    call_id: call_id.clone(),
                    output: truncated,
                }
            }
            ResponseItem::Message { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Other => item.clone(),
        }
    }
}

/// API messages include every non-system item (user/assistant messages, reasoning,
/// tool calls, tool outputs, shell calls, and web-search calls).
fn is_api_message(message: &ResponseItem) -> bool {
    match message {
        ResponseItem::Message { role, .. } => role.as_str() != "system",
        ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => true,
        ResponseItem::GhostSnapshot { .. } => false,
        ResponseItem::Other => false,
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
