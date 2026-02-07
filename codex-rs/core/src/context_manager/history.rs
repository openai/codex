use crate::codex::TurnContext;
use crate::context_manager::normalize;
use crate::instructions::SkillInstructions;
use crate::instructions::UserInstructions;
use crate::session_prefix::is_session_prefix;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::approx_tokens_from_byte_count;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::truncate::truncate_text;
use crate::user_shell_command::is_user_shell_command_text;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use std::ops::Deref;

/// Transcript of thread history
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
    pub(crate) fn record_items<I>(&mut self, items: I, policy: TruncationPolicy)
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

            let processed = self.process_item(item_ref, policy);
            self.items.push(processed);
        }
    }

    /// Returns the history prepared for sending to the model. This applies a proper
    /// normalization and drop un-suited items.
    pub(crate) fn for_prompt(mut self) -> Vec<ResponseItem> {
        self.normalize_history();
        self.items
            .retain(|item| !matches!(item, ResponseItem::GhostSnapshot { .. }));
        self.items
    }

    /// Returns raw items in the history.
    pub(crate) fn raw_items(&self) -> &[ResponseItem] {
        &self.items
    }

    // Estimate token usage using byte-based heuristics from the truncation helpers.
    // This is a coarse lower bound, not a tokenizer-accurate count.
    pub(crate) fn estimate_token_count(&self, turn_context: &TurnContext) -> Option<i64> {
        let model_info = &turn_context.model_info;
        let personality = turn_context.personality.or(turn_context.config.personality);
        let base_instructions = BaseInstructions {
            text: model_info.get_model_instructions(personality),
        };
        self.estimate_token_count_with_base_instructions(&base_instructions)
    }

    pub(crate) fn estimate_token_count_with_base_instructions(
        &self,
        base_instructions: &BaseInstructions,
    ) -> Option<i64> {
        let base_tokens =
            i64::try_from(approx_token_count(&base_instructions.text)).unwrap_or(i64::MAX);

        let items_tokens = self.items.iter().fold(0i64, |acc, item| {
            acc.saturating_add(estimate_item_token_count(item))
        });

        Some(base_tokens.saturating_add(items_tokens))
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

    pub(crate) fn remove_last_item(&mut self) -> bool {
        if let Some(removed) = self.items.pop() {
            normalize::remove_corresponding_for(&mut self.items, &removed);
            true
        } else {
            false
        }
    }

    pub(crate) fn replace(&mut self, items: Vec<ResponseItem>) {
        self.items = items;
    }

    /// Replace image content in the last turn if it originated from a tool output.
    /// Returns true when a tool image was replaced, false otherwise.
    pub(crate) fn replace_last_turn_images(&mut self, placeholder: &str) -> bool {
        let Some(index) = self.items.iter().rposition(|item| {
            matches!(item, ResponseItem::FunctionCallOutput { .. })
                || matches!(item, ResponseItem::Message { role, .. } if role == "user")
        }) else {
            return false;
        };

        match &mut self.items[index] {
            ResponseItem::FunctionCallOutput { output, .. } => {
                let Some(content_items) = output.content_items_mut() else {
                    return false;
                };
                let mut replaced = false;
                let placeholder = placeholder.to_string();
                for item in content_items.iter_mut() {
                    if matches!(item, FunctionCallOutputContentItem::InputImage { .. }) {
                        *item = FunctionCallOutputContentItem::InputText {
                            text: placeholder.clone(),
                        };
                        replaced = true;
                    }
                }
                replaced
            }
            ResponseItem::Message { role, .. } if role == "user" => false,
            _ => false,
        }
    }

    /// Drop the last `num_turns` user turns from this history.
    ///
    /// "User turns" are identified as `ResponseItem::Message` entries whose role is `"user"`.
    ///
    /// This mirrors thread-rollback semantics:
    /// - `num_turns == 0` is a no-op
    /// - if there are no user turns, this is a no-op
    /// - if `num_turns` exceeds the number of user turns, all user turns are dropped while
    ///   preserving any items that occurred before the first user message.
    pub(crate) fn drop_last_n_user_turns(&mut self, num_turns: u32) {
        if num_turns == 0 {
            return;
        }

        let snapshot = self.items.clone();
        let user_positions = user_message_positions(&snapshot);
        let Some(&first_user_idx) = user_positions.first() else {
            self.replace(snapshot);
            return;
        };

        let n_from_end = usize::try_from(num_turns).unwrap_or(usize::MAX);
        let cut_idx = if n_from_end >= user_positions.len() {
            first_user_idx
        } else {
            user_positions[user_positions.len() - n_from_end]
        };

        self.replace(snapshot[..cut_idx].to_vec());
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

    fn get_non_last_reasoning_items_tokens(&self) -> i64 {
        // Get reasoning items excluding all the ones after the last user message.
        let Some(last_user_index) = self
            .items
            .iter()
            .rposition(|item| matches!(item, ResponseItem::Message { role, .. } if role == "user"))
        else {
            return 0;
        };

        self.items
            .iter()
            .take(last_user_index)
            .filter(|item| {
                matches!(
                    item,
                    ResponseItem::Reasoning {
                        encrypted_content: Some(_),
                        ..
                    }
                )
            })
            .fold(0i64, |acc, item| {
                acc.saturating_add(estimate_item_token_count(item))
            })
    }

    fn get_trailing_codex_generated_items_tokens(&self) -> i64 {
        let mut total = 0i64;
        for item in self.items.iter().rev() {
            if !is_codex_generated_item(item) {
                break;
            }
            total = total.saturating_add(estimate_item_token_count(item));
        }
        total
    }

    /// When true, the server already accounted for past reasoning tokens and
    /// the client should not re-estimate them.
    pub(crate) fn get_total_token_usage(&self, server_reasoning_included: bool) -> i64 {
        let last_tokens = self
            .token_info
            .as_ref()
            .map(|info| info.last_token_usage.total_tokens)
            .unwrap_or(0);
        let trailing_codex_generated_tokens = self.get_trailing_codex_generated_items_tokens();
        if server_reasoning_included {
            last_tokens.saturating_add(trailing_codex_generated_tokens)
        } else {
            last_tokens
                .saturating_add(self.get_non_last_reasoning_items_tokens())
                .saturating_add(trailing_codex_generated_tokens)
        }
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

    fn process_item(&self, item: &ResponseItem, policy: TruncationPolicy) -> ResponseItem {
        let policy_with_serialization_budget = policy * 1.2;
        match item {
            ResponseItem::FunctionCallOutput { call_id, output } => {
                let body = match &output.body {
                    FunctionCallOutputBody::Text(content) => FunctionCallOutputBody::Text(
                        truncate_text(content, policy_with_serialization_budget),
                    ),
                    FunctionCallOutputBody::ContentItems(items) => {
                        FunctionCallOutputBody::ContentItems(
                            truncate_function_output_items_with_policy(
                                items,
                                policy_with_serialization_budget,
                            ),
                        )
                    }
                };
                ResponseItem::FunctionCallOutput {
                    call_id: call_id.clone(),
                    output: FunctionCallOutputPayload {
                        body,
                        success: output.success,
                    },
                }
            }
            ResponseItem::CustomToolCallOutput { call_id, output } => {
                let truncated = truncate_text(output, policy_with_serialization_budget);
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
            | ResponseItem::Compaction { .. }
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
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::Compaction { .. } => true,
        ResponseItem::GhostSnapshot { .. } => false,
        ResponseItem::Other => false,
    }
}

fn estimate_reasoning_length(encoded_len: usize) -> usize {
    encoded_len
        .saturating_mul(3)
        .checked_div(4)
        .unwrap_or(0)
        .saturating_sub(650)
}

fn estimate_item_token_count(item: &ResponseItem) -> i64 {
    match item {
        ResponseItem::GhostSnapshot { .. } => 0,
        ResponseItem::Reasoning {
            encrypted_content: Some(content),
            ..
        }
        | ResponseItem::Compaction {
            encrypted_content: content,
        } => {
            let reasoning_bytes = estimate_reasoning_length(content.len());
            i64::try_from(approx_tokens_from_byte_count(reasoning_bytes)).unwrap_or(i64::MAX)
        }
        item => {
            let serialized = serde_json::to_string(item).unwrap_or_default();
            i64::try_from(approx_token_count(&serialized)).unwrap_or(i64::MAX)
        }
    }
}

pub(crate) fn is_codex_generated_item(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. }
    ) || matches!(item, ResponseItem::Message { role, .. } if role == "developer")
}

pub(crate) fn is_user_turn_boundary(item: &ResponseItem) -> bool {
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };

    if role != "user" {
        return false;
    }

    if UserInstructions::is_user_instructions(content)
        || SkillInstructions::is_skill_instructions(content)
    {
        return false;
    }

    for content_item in content {
        match content_item {
            ContentItem::InputText { text } => {
                if is_session_prefix(text) || is_user_shell_command_text(text) {
                    return false;
                }
            }
            ContentItem::OutputText { text } => {
                if is_session_prefix(text) {
                    return false;
                }
            }
            ContentItem::InputImage { .. } => {}
        }
    }

    true
}

fn user_message_positions(items: &[ResponseItem]) -> Vec<usize> {
    let mut positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if is_user_turn_boundary(item) {
            positions.push(idx);
        }
    }
    positions
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
