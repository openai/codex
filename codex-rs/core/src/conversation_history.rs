use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use tracing::error;

/// Transcript of conversation history
#[derive(Debug, Clone, Default)]
pub(crate) struct ConversationHistory {
    /// The oldest items are at the beginning of the vector.
    items: Vec<ResponseItem>,
}

impl ConversationHistory {
    pub(crate) fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub(crate) fn create_with_items(items: Vec<ResponseItem>) -> Self {
        Self { items }
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

    pub(crate) fn remove_last_item(&mut self) {
        self.items.pop();
        self.normalize_history();
    }

    /// This function enforces a couple of invariants on the in-memory history:
    /// 1. every call (function/custom) has a corresponding output entry
    /// 2. every output has a corresponding call entry
    pub(crate) fn normalize_history(&mut self) {
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
        let mut missing_outputs_to_insert: Vec<ResponseItem> = Vec::new();
        for item in &self.items {
            match item {
                ResponseItem::FunctionCall { call_id, .. } => {
                    let has_output = self.items.iter().any(|i| match i {
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error!("Function call output is missing for call id: {}", call_id);
                        missing_outputs_to_insert.push(ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: "aborted".to_string(),
                                success: None,
                            },
                        });
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
                        error!(
                            "Custom tool call output is missing for call id: {}",
                            call_id
                        );
                        missing_outputs_to_insert.push(ResponseItem::CustomToolCallOutput {
                            call_id: call_id.clone(),
                            output: "aborted".to_string(),
                        });
                    }
                }
                // LocalShellCall is represented in upstream streams by a FunctionCallOutput
                ResponseItem::LocalShellCall { call_id, .. } => {
                    if let Some(call_id) = call_id.as_ref() {
                        error!(
                            "Local shell call output is missing for call id: {}",
                            call_id
                        );
                        let has_output = self.items.iter().any(|i| match i {
                            ResponseItem::FunctionCallOutput {
                                call_id: existing, ..
                            } => existing == call_id,
                            _ => false,
                        });

                        if !has_output {
                            missing_outputs_to_insert.push(ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload {
                                    content: "aborted".to_string(),
                                    success: None,
                                },
                            });
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
            self.items.extend(missing_outputs_to_insert);
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
                        _ => false,
                    });

                    if !has_call {
                        error!("Function call is missing for call id: {}", call_id);
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
                        error!("Custom tool call is missing for call id: {}", call_id);
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

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
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
}
