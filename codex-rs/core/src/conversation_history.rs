use codex_protocol::models::ResponseItem;

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

    /// Returns a clone of the contents in the transcript.
    pub(crate) fn contents(&self) -> Vec<ResponseItem> {
        self.items.clone()
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

            // Merge adjacent assistant messages into a single history entry.
            // This prevents duplicates when a partial assistant message was
            // streamed into history earlier in the turn and the final full
            // message is recorded at turn end.
            match (&*item, self.items.last_mut()) {
                (
                    ResponseItem::Message {
                        role: new_role,
                        content: new_content,
                        ..
                    },
                    Some(ResponseItem::Message {
                        role: last_role,
                        content: last_content,
                        ..
                    }),
                ) if new_role == "assistant" && last_role == "assistant" => {
                    append_text_content(last_content, new_content);
                }
                _ => {
                    self.items.push(item.clone());
                }
            }
        }
    }

    /// Append a text `delta` to the latest assistant message, creating a new
    /// assistant entry if none exists yet (e.g. first delta for this turn).
    pub(crate) fn append_assistant_text(&mut self, delta: &str) {
        match self.items.last_mut() {
            Some(ResponseItem::Message { role, content, .. }) if role == "assistant" => {
                append_text_delta(content, delta);
            }
            _ => {
                // Start a new assistant message with the delta.
                self.items.push(ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![codex_protocol::models::ContentItem::OutputText {
                        text: delta.to_string(),
                    }],
                });
            }
        }
    }

    pub(crate) fn keep_last_messages(&mut self, n: usize) {
        if n == 0 {
            self.items.clear();
            return;
        }

        // Collect the last N message items (assistant/user), newest to oldest.
        let mut kept: Vec<ResponseItem> = Vec::with_capacity(n);
        for item in self.items.iter().rev() {
            if let ResponseItem::Message { role, content, .. } = item {
                kept.push(ResponseItem::Message {
                    // we need to remove the id or the model will complain that messages are sent without
                    // their reasonings
                    id: None,
                    role: role.clone(),
                    content: content.clone(),
                });
                if kept.len() == n {
                    break;
                }
            }
        }

        // Preserve chronological order (oldest to newest) within the kept slice.
        kept.reverse();
        self.items = kept;
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
        | ResponseItem::Reasoning { .. } => true,
        ResponseItem::Other => false,
    }
}

/// Helper to append the textual content from `src` into `dst` in place.
fn append_text_content(
    dst: &mut Vec<codex_protocol::models::ContentItem>,
    src: &Vec<codex_protocol::models::ContentItem>,
) {
    for c in src {
        if let codex_protocol::models::ContentItem::OutputText { text } = c {
            append_text_delta(dst, text);
        }
    }
}

/// Append a single text delta to the last OutputText item in `content`, or
/// push a new OutputText item if none exists.
fn append_text_delta(content: &mut Vec<codex_protocol::models::ContentItem>, delta: &str) {
    if let Some(codex_protocol::models::ContentItem::OutputText { text }) = content
        .iter_mut()
        .rev()
        .find(|c| matches!(c, codex_protocol::models::ContentItem::OutputText { .. }))
    {
        text.push_str(delta);
    } else {
        content.push(codex_protocol::models::ContentItem::OutputText {
            text: delta.to_string(),
        });
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
    fn merges_adjacent_assistant_messages() {
        let mut h = ConversationHistory::default();
        let a1 = assistant_msg("Hello");
        let a2 = assistant_msg(", world!");
        h.record_items([&a1, &a2]);

        let items = h.contents();
        assert_eq!(
            items,
            vec![ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hello, world!".to_string()
                }]
            }]
        );
    }

    #[test]
    fn append_assistant_text_creates_and_appends() {
        let mut h = ConversationHistory::default();
        h.append_assistant_text("Hello");
        h.append_assistant_text(", world");

        // Now record a final full assistant message and verify it merges.
        let final_msg = assistant_msg("!");
        h.record_items([&final_msg]);

        let items = h.contents();
        assert_eq!(
            items,
            vec![ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "Hello, world!".to_string()
                }]
            }]
        );
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
