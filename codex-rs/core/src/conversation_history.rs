use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;

use crate::util::error_or_panic;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;
use std::ops::Deref;

// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
pub(crate) const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub(crate) const MODEL_FORMAT_MAX_LINES: usize = 256; // lines
pub(crate) const MODEL_FORMAT_HEAD_LINES: usize = MODEL_FORMAT_MAX_LINES / 2;
pub(crate) const MODEL_FORMAT_TAIL_LINES: usize = MODEL_FORMAT_MAX_LINES - MODEL_FORMAT_HEAD_LINES; // 128
pub(crate) const MODEL_FORMAT_HEAD_BYTES: usize = MODEL_FORMAT_MAX_BYTES / 2;

/// Transcript of conversation history
#[derive(Debug, Clone, Default)]
pub(crate) struct ConversationHistory {
    /// The oldest items are at the beginning of the vector.
    items: Vec<ResponseItem>,
    token_info: Option<TokenUsageInfo>,
}

impl ConversationHistory {
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
                ResponseItem::Reasoning { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::FunctionCallOutput { .. }
                | ResponseItem::CustomToolCallOutput { .. }
                | ResponseItem::GhostSnapshot { .. }
                | ResponseItem::Other
                | ResponseItem::Message { .. } => {
                    // nothing to do for these variants
                }
            }
        }

        if !missing_outputs_to_insert.is_empty() {
            // Insert from the end to avoid shifting subsequent indices.
            missing_outputs_to_insert.sort_by_key(|(i, _)| *i);
            for (idx, item) in missing_outputs_to_insert.into_iter().rev() {
                let insert_pos = idx + 1; // place immediately after the call
                if insert_pos <= self.items.len() {
                    self.items.insert(insert_pos, item);
                } else {
                    self.items.push(item);
                }
            }
        }
    }

    fn remove_orphan_outputs(&mut self) {
        // Work on a snapshot to avoid borrowing `self.items` while mutating it.
        let snapshot = self.items.clone();
        let mut orphan_output_call_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for item in &snapshot {
            match item {
                ResponseItem::FunctionCallOutput { call_id, .. } => {
                    let has_call = snapshot.iter().any(|i| match i {
                        ResponseItem::FunctionCall {
                            call_id: existing, ..
                        } => existing == call_id,
                        ResponseItem::LocalShellCall {
                            call_id: Some(existing),
                            ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_call {
                        error_or_panic(format!("Function call is missing for call id: {call_id}"));
                        orphan_output_call_ids.insert(call_id.clone());
                    }
                }
                ResponseItem::CustomToolCallOutput { call_id, .. } => {
                    let has_call = snapshot.iter().any(|i| match i {
                        ResponseItem::CustomToolCall {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_call {
                        error_or_panic(format!(
                            "Custom tool call is missing for call id: {call_id}"
                        ));
                        orphan_output_call_ids.insert(call_id.clone());
                    }
                }
                ResponseItem::FunctionCall { .. }
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::Reasoning { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::GhostSnapshot { .. }
                | ResponseItem::Other
                | ResponseItem::Message { .. } => {
                    // nothing to do for these variants
                }
            }
        }

        if !orphan_output_call_ids.is_empty() {
            let ids = orphan_output_call_ids;
            self.items.retain(|i| match i {
                ResponseItem::FunctionCallOutput { call_id, .. }
                | ResponseItem::CustomToolCallOutput { call_id, .. } => !ids.contains(call_id),
                _ => true,
            });
        }
    }

    /// Removes the corresponding paired item for the provided `item`, if any.
    ///
    /// Pairs:
    /// - FunctionCall <-> FunctionCallOutput
    /// - CustomToolCall <-> CustomToolCallOutput
    /// - LocalShellCall(call_id: Some) <-> FunctionCallOutput
    fn remove_corresponding_for(&mut self, item: &ResponseItem) {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                self.remove_first_matching(|i| match i {
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                self.remove_first_matching(|i| match i {
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });
            }
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => {
                self.remove_first_matching(|i| match i {
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });
            }
            ResponseItem::FunctionCallOutput { call_id, .. } => {
                self.remove_first_matching(|i| match i {
                    ResponseItem::FunctionCall {
                        call_id: existing, ..
                    } => existing == call_id,
                    ResponseItem::LocalShellCall {
                        call_id: Some(existing),
                        ..
                    } => existing == call_id,
                    _ => false,
                });
            }
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                self.remove_first_matching(|i| match i {
                    ResponseItem::CustomToolCall {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });
            }
            _ => {}
        }
    }

    /// Remove the first item matching the predicate.
    fn remove_first_matching<F>(&mut self, predicate: F)
    where
        F: FnMut(&ResponseItem) -> bool,
    {
        if let Some(pos) = self.items.iter().position(predicate) {
            self.items.remove(pos);
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

fn globally_truncate_function_output_items(
    items: &[FunctionCallOutputContentItem],
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining = MODEL_FORMAT_MAX_BYTES;
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let len = text.len();
                if len <= remaining {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining -= len;
                } else {
                    let slice = take_bytes_at_char_boundary(text, remaining);
                    if !slice.is_empty() {
                        out.push(FunctionCallOutputContentItem::InputText {
                            text: slice.to_string(),
                        });
                    }
                    remaining = 0;
                }
            }
            // todo(aibrahim): handle input images; resize
            FunctionCallOutputContentItem::InputImage { image_url } => {
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                });
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }

    out
}

pub(crate) fn format_output_for_model_body(content: &str) -> String {
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= MODEL_FORMAT_MAX_BYTES && total_lines <= MODEL_FORMAT_MAX_LINES {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines);
    format!("Total output lines: {total_lines}\n\n{output}")
}

fn truncate_formatted_exec_output(content: &str, total_lines: usize) -> String {
    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = MODEL_FORMAT_HEAD_LINES.min(segments.len());
    let tail_take = MODEL_FORMAT_TAIL_LINES.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();
    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };
    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];
    let truncated_by_bytes = content.len() > MODEL_FORMAT_MAX_BYTES;
    // this is a bit wrong. We are counting metadata lines and not just shell output lines.
    let marker = if omitted > 0 {
        Some(format!(
            "\n[... omitted {omitted} of {total_lines} lines ...]\n\n"
        ))
    } else if truncated_by_bytes {
        Some(format!(
            "\n[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]\n\n"
        ))
    } else {
        None
    };

    let marker_len = marker.as_ref().map_or(0, String::len);
    let base_head_budget = MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES);
    let head_budget = base_head_budget.min(MODEL_FORMAT_MAX_BYTES.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(content.len()));

    result.push_str(head_part);
    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
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
mod tests {
    use super::*;
    use codex_git::GhostCommit;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::LocalShellAction;
    use codex_protocol::models::LocalShellExecAction;
    use codex_protocol::models::LocalShellStatus;
    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use pretty_assertions::assert_eq;

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    fn create_history_with_items(items: Vec<ResponseItem>) -> ConversationHistory {
        let mut h = ConversationHistory::new();
        h.record_items(items.iter());
        h
    }

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    fn reasoning_msg(text: &str) -> ResponseItem {
        ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "summary".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: text.to_string(),
            }]),
            encrypted_content: None,
        }
    }

    #[test]
    fn filters_non_api_messages() {
        let mut h = ConversationHistory::default();
        // System message is not API messages; Other is ignored.
        let system = ResponseItem::Message {
            id: None,
            role: "system".to_string(),
            content: vec![ContentItem::OutputText {
                text: "ignored".to_string(),
            }],
        };
        let reasoning = reasoning_msg("thinking...");
        h.record_items([&system, &reasoning, &ResponseItem::Other]);

        // User and assistant should be retained.
        let u = user_msg("hi");
        let a = assistant_msg("hello");
        h.record_items([&u, &a]);

        let items = h.contents();
        assert_eq!(
            items,
            vec![
                ResponseItem::Reasoning {
                    id: String::new(),
                    summary: vec![ReasoningItemReasoningSummary::SummaryText {
                        text: "summary".to_string(),
                    }],
                    content: Some(vec![ReasoningItemContent::ReasoningText {
                        text: "thinking...".to_string(),
                    }]),
                    encrypted_content: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "hi".to_string()
                    }]
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "hello".to_string()
                    }]
                }
            ]
        );
    }

    #[test]
    fn get_history_for_prompt_drops_ghost_commits() {
        let items = vec![ResponseItem::GhostSnapshot {
            ghost_commit: GhostCommit::new("ghost-1".to_string(), None, Vec::new(), Vec::new()),
        }];
        let mut history = create_history_with_items(items);
        let filtered = history.get_history_for_prompt();
        assert_eq!(filtered, vec![]);
    }

    #[test]
    fn remove_first_item_removes_matching_output_for_function_call() {
        let items = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload {
                    content: "ok".to_string(),
                    ..Default::default()
                },
            },
        ];
        let mut h = create_history_with_items(items);
        h.remove_first_item();
        assert_eq!(h.contents(), vec![]);
    }

    #[test]
    fn remove_first_item_removes_matching_call_for_output() {
        let items = vec![
            ResponseItem::FunctionCallOutput {
                call_id: "call-2".to_string(),
                output: FunctionCallOutputPayload {
                    content: "ok".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "do_it".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-2".to_string(),
            },
        ];
        let mut h = create_history_with_items(items);
        h.remove_first_item();
        assert_eq!(h.contents(), vec![]);
    }

    #[test]
    fn remove_first_item_handles_local_shell_pair() {
        let items = vec![
            ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("call-3".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string(), "hi".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                }),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-3".to_string(),
                output: FunctionCallOutputPayload {
                    content: "ok".to_string(),
                    ..Default::default()
                },
            },
        ];
        let mut h = create_history_with_items(items);
        h.remove_first_item();
        assert_eq!(h.contents(), vec![]);
    }

    #[test]
    fn remove_first_item_handles_custom_tool_pair() {
        let items = vec![
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "tool-1".to_string(),
                name: "my_tool".to_string(),
                input: "{}".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "tool-1".to_string(),
                output: "ok".to_string(),
            },
        ];
        let mut h = create_history_with_items(items);
        h.remove_first_item();
        assert_eq!(h.contents(), vec![]);
    }

    #[test]
    fn record_items_truncates_function_call_output_content() {
        let mut history = ConversationHistory::new();
        let long_line = "a very long line to trigger truncation\n";
        let long_output = long_line.repeat(2_500);
        let item = ResponseItem::FunctionCallOutput {
            call_id: "call-100".to_string(),
            output: FunctionCallOutputPayload {
                content: long_output.clone(),
                success: Some(true),
                ..Default::default()
            },
        };

        history.record_items([&item]);

        assert_eq!(history.items.len(), 1);
        match &history.items[0] {
            ResponseItem::FunctionCallOutput { output, .. } => {
                assert_ne!(output.content, long_output);
                assert!(
                    output.content.starts_with("Total output lines:"),
                    "expected truncated summary, got {}",
                    output.content
                );
            }
            other => panic!("unexpected history item: {other:?}"),
        }
    }

    #[test]
    fn record_items_truncates_custom_tool_call_output_content() {
        let mut history = ConversationHistory::new();
        let line = "custom output that is very long\n";
        let long_output = line.repeat(2_500);
        let item = ResponseItem::CustomToolCallOutput {
            call_id: "tool-200".to_string(),
            output: long_output.clone(),
        };

        history.record_items([&item]);

        assert_eq!(history.items.len(), 1);
        match &history.items[0] {
            ResponseItem::CustomToolCallOutput { output, .. } => {
                assert_ne!(output, &long_output);
                assert!(
                    output.starts_with("Total output lines:"),
                    "expected truncated summary, got {output}"
                );
            }
            other => panic!("unexpected history item: {other:?}"),
        }
    }

    // The following tests were adapted from tools::mod truncation tests to
    // target the new truncation functions in conversation_history.

    use regex_lite::Regex;

    fn assert_truncated_message_matches(message: &str, line: &str, total_lines: usize) {
        let pattern = truncated_message_pattern(line, total_lines);
        let regex = Regex::new(&pattern).unwrap_or_else(|err| {
            panic!("failed to compile regex {pattern}: {err}");
        });
        let captures = regex
            .captures(message)
            .unwrap_or_else(|| panic!("message failed to match pattern {pattern}: {message}"));
        let body = captures
            .name("body")
            .expect("missing body capture")
            .as_str();
        assert!(
            body.len() <= MODEL_FORMAT_MAX_BYTES,
            "body exceeds byte limit: {} bytes",
            body.len()
        );
    }

    fn truncated_message_pattern(line: &str, total_lines: usize) -> String {
        let head_take = MODEL_FORMAT_HEAD_LINES.min(total_lines);
        let tail_take = MODEL_FORMAT_TAIL_LINES.min(total_lines.saturating_sub(head_take));
        let omitted = total_lines.saturating_sub(head_take + tail_take);
        let escaped_line = regex_lite::escape(line);
        if omitted == 0 {
            return format!(
                r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes \.{{3}}]\n\n.*)$",
            );
        }
        format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
        )
    }

    #[test]
    fn format_exec_output_truncates_large_error() {
        let line = "very long execution error line that should trigger truncation\n";
        let large_error = line.repeat(2_500); // way beyond both byte and line limits

        let truncated = format_output_for_model_body(&large_error);

        let total_lines = large_error.lines().count();
        assert_truncated_message_matches(&truncated, line, total_lines);
        assert_ne!(truncated, large_error);
    }

    #[test]
    fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
        let long_line = "a".repeat(MODEL_FORMAT_MAX_BYTES + 50);
        let truncated = format_output_for_model_body(&long_line);

        assert_ne!(truncated, long_line);
        let marker_line =
            format!("[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]");
        assert!(
            truncated.contains(&marker_line),
            "missing byte truncation marker: {truncated}"
        );
        assert!(
            !truncated.contains("omitted"),
            "line omission marker should not appear when no lines were dropped: {truncated}"
        );
    }

    #[test]
    fn format_exec_output_returns_original_when_within_limits() {
        let content = "example output\n".repeat(10);

        assert_eq!(format_output_for_model_body(&content), content);
    }

    #[test]
    fn format_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 100;
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}\n"))
            .collect();

        let truncated = format_output_for_model_body(&content);
        let omitted = total_lines - MODEL_FORMAT_MAX_LINES;
        let expected_marker = format!("[... omitted {omitted} of {total_lines} lines ...]");

        assert!(
            truncated.contains(&expected_marker),
            "missing omitted marker: {truncated}"
        );
        assert!(
            truncated.contains("line-0\n"),
            "expected head line to remain: {truncated}"
        );

        let last_line = format!("line-{}\n", total_lines - 1);
        assert!(
            truncated.contains(&last_line),
            "expected tail line to remain: {truncated}"
        );
    }

    #[test]
    fn format_exec_output_prefers_line_marker_when_both_limits_exceeded() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 42;
        let long_line = "x".repeat(256);
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}-{long_line}\n"))
            .collect();

        let truncated = format_output_for_model_body(&content);

        assert!(
            truncated.contains("[... omitted 42 of 298 lines ...]"),
            "expected omitted marker when line count exceeds limit: {truncated}"
        );
        assert!(
            !truncated.contains("output truncated to fit"),
            "line omission marker should take precedence over byte marker: {truncated}"
        );
    }

    #[test]
    fn truncates_across_multiple_under_limit_texts_and_reports_omitted() {
        // Arrange: several text items, none exceeding per-item limit, but total exceeds budget.
        let budget = MODEL_FORMAT_MAX_BYTES;
        let t1_len = (budget / 2).saturating_sub(10);
        let t2_len = (budget / 2).saturating_sub(10);
        let remaining_after_t1_t2 = budget.saturating_sub(t1_len + t2_len);
        let t3_len = 50; // gets truncated to remaining_after_t1_t2
        let t4_len = 5; // omitted
        let t5_len = 7; // omitted

        let t1 = "a".repeat(t1_len);
        let t2 = "b".repeat(t2_len);
        let t3 = "c".repeat(t3_len);
        let t4 = "d".repeat(t4_len);
        let t5 = "e".repeat(t5_len);

        let item = ResponseItem::FunctionCallOutput {
            call_id: "call-omit".to_string(),
            output: FunctionCallOutputPayload {
                content: "irrelevant".to_string(),
                content_items: Some(vec![
                    FunctionCallOutputContentItem::InputText { text: t1 },
                    FunctionCallOutputContentItem::InputText { text: t2 },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "img:mid".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText { text: t3 },
                    FunctionCallOutputContentItem::InputText { text: t4 },
                    FunctionCallOutputContentItem::InputText { text: t5 },
                ]),
                success: Some(true),
            },
        };

        let mut history = ConversationHistory::new();
        history.record_items([&item]);
        assert_eq!(history.items.len(), 1);
        let json = serde_json::to_value(&history.items[0]).expect("serialize to json");

        let output = json
            .get("output")
            .expect("output field")
            .as_array()
            .expect("array output");

        // Expect: t1 (full), t2 (full), image, t3 (truncated), summary mentioning 2 omitted.
        assert_eq!(output.len(), 5);

        let first = output[0].as_object().expect("first obj");
        assert_eq!(first.get("type").unwrap(), "input_text");
        let first_text = first.get("text").unwrap().as_str().unwrap();
        assert_eq!(first_text.len(), t1_len);

        let second = output[1].as_object().expect("second obj");
        assert_eq!(second.get("type").unwrap(), "input_text");
        let second_text = second.get("text").unwrap().as_str().unwrap();
        assert_eq!(second_text.len(), t2_len);

        assert_eq!(
            output[2],
            serde_json::json!({"type": "input_image", "image_url": "img:mid"})
        );

        let fourth = output[3].as_object().expect("fourth obj");
        assert_eq!(fourth.get("type").unwrap(), "input_text");
        let fourth_text = fourth.get("text").unwrap().as_str().unwrap();
        assert_eq!(fourth_text.len(), remaining_after_t1_t2);

        let summary = output[4].as_object().expect("summary obj");
        assert_eq!(summary.get("type").unwrap(), "input_text");
        let summary_text = summary.get("text").unwrap().as_str().unwrap();
        assert!(summary_text.contains("omitted 2 text items"));
    }

    //TODO(aibrahim): run CI in release mode.
    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_adds_missing_output_for_function_call() {
        let items = vec![ResponseItem::FunctionCall {
            id: None,
            name: "do_it".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-x".to_string(),
        }];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(
            h.contents(),
            vec![
                ResponseItem::FunctionCall {
                    id: None,
                    name: "do_it".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "call-x".to_string(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-x".to_string(),
                    output: FunctionCallOutputPayload {
                        content: "aborted".to_string(),
                        ..Default::default()
                    },
                },
            ]
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_adds_missing_output_for_custom_tool_call() {
        let items = vec![ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "tool-x".to_string(),
            name: "custom".to_string(),
            input: "{}".to_string(),
        }];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(
            h.contents(),
            vec![
                ResponseItem::CustomToolCall {
                    id: None,
                    status: None,
                    call_id: "tool-x".to_string(),
                    name: "custom".to_string(),
                    input: "{}".to_string(),
                },
                ResponseItem::CustomToolCallOutput {
                    call_id: "tool-x".to_string(),
                    output: "aborted".to_string(),
                },
            ]
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_adds_missing_output_for_local_shell_call_with_id() {
        let items = vec![ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hi".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        }];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(
            h.contents(),
            vec![
                ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some("shell-1".to_string()),
                    status: LocalShellStatus::Completed,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["echo".to_string(), "hi".to_string()],
                        timeout_ms: None,
                        working_directory: None,
                        env: None,
                        user: None,
                    }),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "shell-1".to_string(),
                    output: FunctionCallOutputPayload {
                        content: "aborted".to_string(),
                        ..Default::default()
                    },
                },
            ]
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_removes_orphan_function_call_output() {
        let items = vec![ResponseItem::FunctionCallOutput {
            call_id: "orphan-1".to_string(),
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
        }];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(h.contents(), vec![]);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_removes_orphan_custom_tool_call_output() {
        let items = vec![ResponseItem::CustomToolCallOutput {
            call_id: "orphan-2".to_string(),
            output: "ok".to_string(),
        }];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(h.contents(), vec![]);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn normalize_mixed_inserts_and_removals() {
        let items = vec![
            // Will get an inserted output
            ResponseItem::FunctionCall {
                id: None,
                name: "f1".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            // Orphan output that should be removed
            ResponseItem::FunctionCallOutput {
                call_id: "c2".to_string(),
                output: FunctionCallOutputPayload {
                    content: "ok".to_string(),
                    ..Default::default()
                },
            },
            // Will get an inserted custom tool output
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "t1".to_string(),
                name: "tool".to_string(),
                input: "{}".to_string(),
            },
            // Local shell call also gets an inserted function call output
            ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("s1".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                }),
            },
        ];
        let mut h = create_history_with_items(items);

        h.normalize_history();

        assert_eq!(
            h.contents(),
            vec![
                ResponseItem::FunctionCall {
                    id: None,
                    name: "f1".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "c1".to_string(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "c1".to_string(),
                    output: FunctionCallOutputPayload {
                        content: "aborted".to_string(),
                        ..Default::default()
                    },
                },
                ResponseItem::CustomToolCall {
                    id: None,
                    status: None,
                    call_id: "t1".to_string(),
                    name: "tool".to_string(),
                    input: "{}".to_string(),
                },
                ResponseItem::CustomToolCallOutput {
                    call_id: "t1".to_string(),
                    output: "aborted".to_string(),
                },
                ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some("s1".to_string()),
                    status: LocalShellStatus::Completed,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["echo".to_string()],
                        timeout_ms: None,
                        working_directory: None,
                        env: None,
                        user: None,
                    }),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "s1".to_string(),
                    output: FunctionCallOutputPayload {
                        content: "aborted".to_string(),
                        ..Default::default()
                    },
                },
            ]
        );
    }

    // In debug builds we panic on normalization errors instead of silently fixing them.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_adds_missing_output_for_function_call_panics_in_debug() {
        let items = vec![ResponseItem::FunctionCall {
            id: None,
            name: "do_it".to_string(),
            arguments: "{}".to_string(),
            call_id: "call-x".to_string(),
        }];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_adds_missing_output_for_custom_tool_call_panics_in_debug() {
        let items = vec![ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "tool-x".to_string(),
            name: "custom".to_string(),
            input: "{}".to_string(),
        }];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_adds_missing_output_for_local_shell_call_with_id_panics_in_debug() {
        let items = vec![ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".to_string(), "hi".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
        }];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_removes_orphan_function_call_output_panics_in_debug() {
        let items = vec![ResponseItem::FunctionCallOutput {
            call_id: "orphan-1".to_string(),
            output: FunctionCallOutputPayload {
                content: "ok".to_string(),
                ..Default::default()
            },
        }];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_removes_orphan_custom_tool_call_output_panics_in_debug() {
        let items = vec![ResponseItem::CustomToolCallOutput {
            call_id: "orphan-2".to_string(),
            output: "ok".to_string(),
        }];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn normalize_mixed_inserts_and_removals_panics_in_debug() {
        let items = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "f1".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "c2".to_string(),
                output: FunctionCallOutputPayload {
                    content: "ok".to_string(),
                    ..Default::default()
                },
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "t1".to_string(),
                name: "tool".to_string(),
                input: "{}".to_string(),
            },
            ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("s1".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string()],
                    timeout_ms: None,
                    working_directory: None,
                    env: None,
                    user: None,
                }),
            },
        ];
        let mut h = create_history_with_items(items);
        h.normalize_history();
    }
}
