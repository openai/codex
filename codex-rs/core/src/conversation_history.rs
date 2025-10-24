use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use tracing::error;

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
            if !is_api_message(&item) {
                continue;
            }

            self.items.push(item.clone());
        }
    }

    pub(crate) fn get_history(&mut self) -> Vec<ResponseItem> {
        self.normalize_history();
        self.contents()
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
                                    success: None,
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
                                        success: None,
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

    pub(crate) fn replace(&mut self, items: Vec<ResponseItem>) {
        self.items = items;
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
}

#[inline]
fn error_or_panic(message: String) {
    if cfg!(debug_assertions) || env!("CARGO_PKG_VERSION").contains("alpha") {
        panic!("{message}");
    } else {
        error!("{message}");
    }
}

/// Anything that is not a system message or "reasoning" message is considered
/// an API message.
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
        ResponseItem::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::LocalShellAction;
    use codex_protocol::models::LocalShellExecAction;
    use codex_protocol::models::LocalShellStatus;
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

    #[test]
    fn filters_non_api_messages() {
        let mut h = ConversationHistory::default();
        // System message is not an API message; Other is ignored.
        let system = ResponseItem::Message {
            id: None,
            role: "system".to_string(),
            content: vec![ContentItem::OutputText {
                text: "ignored".to_string(),
            }],
        };
        h.record_items([&system, &ResponseItem::Other]);

        // User and assistant should be retained.
        let u = user_msg("hi");
        let a = assistant_msg("hello");
        h.record_items([&u, &a]);

        let items = h.contents();
        assert_eq!(
            items,
            vec![
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
                    success: None,
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
                    success: None,
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
                    success: None,
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
                        success: None,
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
                        success: None,
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
                success: None,
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
                    success: None,
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
                        success: None,
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
                        success: None,
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
                success: None,
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
                    success: None,
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
