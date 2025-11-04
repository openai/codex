use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use std::ops::Deref;

use crate::util::error_or_panic;

use crate::context_manager::truncate::format_output_for_model_body;
use crate::context_manager::truncate::globally_truncate_function_output_items;

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

    pub(crate) fn remove_first_item(&mut self) {
        if !self.items.is_empty() {
            // Remove the oldest item (front of the list). Items are ordered from
            // oldest → newest, so index 0 is the first entry recorded.
            let removed = self.items.remove(0);
            // If the removed item participates in a call/output pair, also remove
            // its corresponding counterpart to keep the invariants intact without
            // running a full normalization pass.
            self.remove_corresponding_for(&removed);
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
        self.ensure_call_outputs_present();

        // all outputs must have a corresponding function/tool call
        self.remove_orphan_outputs();
    }

    /// Returns a clone of the contents in the transcript.
    fn contents(&self) -> Vec<ResponseItem> {
        self.items.clone()
    }

    fn remove_ghost_snapshots(items: &mut Vec<ResponseItem>) {
        items.retain(|item| !matches!(item, ResponseItem::GhostSnapshot { .. }));
    }

    fn ensure_call_outputs_present(&mut self) {
        // Collect synthetic outputs to insert immediately after their calls.
        // Store the insertion position (index of call) alongside the item so
        // we can insert in reverse order and avoid index shifting.
        let mut missing_outputs_to_insert: Vec<(usize, ResponseItem)> = Vec::new();

        for (idx, item) in self.items.iter().enumerate() {
            match item {
                ResponseItem::FunctionCall { call_id, .. } => {
                    let has_output = self.items.iter().any(|i| match i {
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error_or_panic(format!(
                            "Function call output is missing for call id: {call_id}"
                        ));
                        missing_outputs_to_insert.push((
                            idx,
                            ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload {
                                    content: "aborted".to_string(),
                                    ..Default::default()
                                },
                            },
                        ));
                    }
                }
                ResponseItem::CustomToolCall { call_id, .. } => {
                    let has_output = self.items.iter().any(|i| match i {
                        ResponseItem::CustomToolCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error_or_panic(format!(
                            "Custom tool call output is missing for call id: {call_id}"
                        ));
                        missing_outputs_to_insert.push((
                            idx,
                            ResponseItem::CustomToolCallOutput {
                                call_id: call_id.clone(),
                                output: "aborted".to_string(),
                            },
                        ));
                    }
                }
                // LocalShellCall is represented in upstream streams by a FunctionCallOutput
                ResponseItem::LocalShellCall { call_id, .. } => {
                    if let Some(call_id) = call_id.as_ref() {
                        let has_output = self.items.iter().any(|i| match i {
                            ResponseItem::FunctionCallOutput {
                                call_id: existing, ..
                            } => existing == call_id,
                            _ => false,
                        });

                        if !has_output {
                            error_or_panic(format!(
                                "Local shell call output is missing for call id: {call_id}"
                            ));
                            missing_outputs_to_insert.push((
                                idx,
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: "aborted".to_string(),
                                        ..Default::default()
                                    },
                                },
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Insert synthetic outputs in reverse index order to avoid re-indexing.
        for (idx, output_item) in missing_outputs_to_insert.into_iter().rev() {
            self.items.insert(idx + 1, output_item);
        }
    }

    fn remove_orphan_outputs(&mut self) {
        use std::collections::HashSet;

        let function_call_ids: HashSet<String> = self
            .items
            .iter()
            .filter_map(|i| match i {
                ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
                _ => None,
            })
            .collect();

        let custom_tool_call_ids: HashSet<String> = self
            .items
            .iter()
            .filter_map(|i| match i {
                ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
                _ => None,
            })
            .collect();

        self.items.retain(|item| match item {
            ResponseItem::FunctionCallOutput { call_id, .. } => function_call_ids.contains(call_id),
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                custom_tool_call_ids.contains(call_id)
            }
            _ => true,
        });
    }

    fn remove_corresponding_for(&mut self, item: &ResponseItem) {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                Self::remove_first_matching(&mut self.items, |i| {
                    matches!(
                        i,
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } if existing == call_id
                    )
                });
            }
            ResponseItem::FunctionCallOutput { call_id, .. } => {
                Self::remove_first_matching(
                    &mut self.items,
                    |i| matches!(i, ResponseItem::FunctionCall { call_id: existing, .. } if existing == call_id),
                );
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                Self::remove_first_matching(&mut self.items, |i| {
                    matches!(
                        i,
                        ResponseItem::CustomToolCallOutput {
                            call_id: existing, ..
                        } if existing == call_id
                    )
                });
            }
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                Self::remove_first_matching(
                    &mut self.items,
                    |i| matches!(i, ResponseItem::CustomToolCall { call_id: existing, .. } if existing == call_id),
                );
            }
            ResponseItem::LocalShellCall { call_id, .. } => {
                if let Some(call_id) = call_id {
                    Self::remove_first_matching(&mut self.items, |i| {
                        matches!(
                            i,
                            ResponseItem::FunctionCallOutput {
                                call_id: existing, ..
                            } if existing == call_id
                        )
                    });
                }
            }
            _ => {}
        }
    }

    fn remove_first_matching<F>(items: &mut Vec<ResponseItem>, predicate: F)
    where
        F: Fn(&ResponseItem) -> bool,
    {
        if let Some(pos) = items.iter().position(predicate) {
            items.remove(pos);
        }
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
