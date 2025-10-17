use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;

use crate::state::TaskKind;

/// Transcript of conversation history
#[derive(Debug, Clone, Default)]
pub(crate) struct ConversationHistory {
    /// The oldest items are at the beginning of the vector.
    items: Vec<ResponseItem>,
    review_thread_history: Vec<ResponseItem>,
}

impl ConversationHistory {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            review_thread_history: Vec::new(),
        }
    }

    /// Returns a clone of the contents in the transcript.
    pub(crate) fn contents(&self) -> Vec<ResponseItem> {
        self.items.clone()
    }

    pub(crate) fn review_thread_contents(&self) -> Vec<ResponseItem> {
        self.review_thread_history.clone()
    }

    pub(crate) fn clear_review_thread(&mut self) {
        self.review_thread_history.clear();
    }

    /// `items` is ordered from oldest to newest.
    pub(crate) fn record_items<I>(&mut self, items: I, task_kind: TaskKind)
    where
        I: IntoIterator<Item = ResponseItem>,
    {
        for item in items {
            if !is_api_message(&item) {
                continue;
            }

            match task_kind {
                TaskKind::Regular | TaskKind::Compact => {
                    self.items.push(item);
                }
                TaskKind::Review => {
                    self.review_thread_history.push(item);
                }
            }
        }
    }

    pub(crate) fn replace(&mut self, items: Vec<ResponseItem>) {
        self.items = items;
    }

    pub(crate) fn initialize_review_history(
        &mut self,
        response_input: &ResponseInputItem,
        initial_context: Vec<ResponseItem>,
    ) {
        self.clear_review_thread();
        self.record_items(initial_context, TaskKind::Review);
        self.record_items(
            std::iter::once(ResponseItem::from(response_input.clone())),
            TaskKind::Review,
        );
    }

    pub(crate) fn add_pending_input(
        &mut self,
        pending_input: Vec<ResponseItem>,
        task_kind: TaskKind,
    ) {
        self.record_items(pending_input, task_kind);
    }

    pub(crate) fn handle_missing_tool_call_output(&mut self, task_kind: TaskKind) {
        // call_ids that are part of this response.
        let content = match task_kind {
            TaskKind::Regular => self.contents(),
            TaskKind::Review => self.review_thread_contents(),
            TaskKind::Compact => self.contents(),
        };
        let completed_call_ids = content
            .iter()
            .filter_map(|ri| match ri {
                ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id),
                ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id),
                _ => None,
            })
            .collect::<Vec<_>>();

        // call_ids that were pending but are not part of this response.
        // This usually happens because the user interrupted the model before we responded to one of its tool calls
        // and then the user sent a follow-up message.
        let missing_calls = {
            content
                .iter()
                .filter_map(|ri| match ri {
                    ResponseItem::FunctionCall { call_id, .. } => Some(call_id),
                    ResponseItem::LocalShellCall {
                        call_id: Some(call_id),
                        ..
                    } => Some(call_id),
                    ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
                    _ => None,
                })
                .filter_map(|call_id| {
                    if completed_call_ids.contains(&call_id) {
                        None
                    } else {
                        Some(call_id.clone())
                    }
                })
                .map(|call_id| ResponseItem::CustomToolCallOutput {
                    call_id,
                    output: "aborted".to_string(),
                })
                .collect::<Vec<_>>()
        };
        self.record_items(missing_calls, task_kind);
    }

    pub(crate) fn prompt(&self, task_kind: TaskKind) -> Vec<ResponseItem> {
        match task_kind {
            TaskKind::Regular | TaskKind::Compact => self.contents(),
            TaskKind::Review => self.review_thread_contents(),
        }
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
        h.record_items([system, ResponseItem::Other], TaskKind::Regular);

        // User and assistant should be retained.
        let u = user_msg("hi");
        let a = assistant_msg("hello");
        h.record_items([u, a], TaskKind::Regular);

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
            ],
        );
    }
}
