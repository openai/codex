use crate::protocol::AgentMessageEvent;
use crate::protocol::AgentReasoningEvent;
use crate::protocol::AgentReasoningRawContentEvent;
use crate::protocol::EventMsg;
use crate::protocol::InputMessageKind;
use crate::protocol::UserMessageEvent;
use crate::protocol::WebSearchEndEvent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;

/// Convert a `ResponseItem` into zero or more `EventMsg` values that the UI can render.
///
/// When `show_raw_agent_reasoning` is false, raw reasoning content events are omitted.
pub(crate) fn map_response_item_to_event_messages(
    item: &ResponseItem,
    show_raw_agent_reasoning: bool,
) -> Vec<EventMsg> {
    match item {
        ResponseItem::Message { role, content, .. } => {
            // Do not surface system messages as user events.
            if role == "system" {
                return Vec::new();
            }

            let mut events: Vec<EventMsg> = Vec::new();
            let mut message_parts: Vec<String> = Vec::new();
            let mut images: Vec<String> = Vec::new();
            let mut kind: Option<InputMessageKind> = None;

            for content_item in content.iter() {
                match content_item {
                    ContentItem::InputText { text } => {
                        if kind.is_none() {
                            let trimmed = text.trim_start();
                            kind = if trimmed.starts_with("<environment_context>") {
                                Some(InputMessageKind::EnvironmentContext)
                            } else if trimmed.starts_with("<user_instructions>") {
                                Some(InputMessageKind::UserInstructions)
                            } else {
                                Some(InputMessageKind::Plain)
                            };
                        }
                        message_parts.push(text.clone());
                    }
                    ContentItem::InputImage { image_url } => {
                        images.push(image_url.clone());
                    }
                    ContentItem::OutputText { text } => {
                        events.push(EventMsg::AgentMessage(AgentMessageEvent {
                            message: text.clone(),
                        }));
                    }
                }
            }

            if !message_parts.is_empty() || !images.is_empty() {
                let message = if message_parts.is_empty() {
                    String::new()
                } else {
                    message_parts.join("")
                };
                let images = if images.is_empty() {
                    None
                } else {
                    Some(images)
                };

                events.push(EventMsg::UserMessage(UserMessageEvent {
                    message,
                    kind,
                    images,
                }));
            }

            events
        }

        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            let mut events = Vec::new();
            for ReasoningItemReasoningSummary::SummaryText { text } in summary {
                events.push(EventMsg::AgentReasoning(AgentReasoningEvent {
                    text: text.clone(),
                }));
            }
            if let Some(items) = content.as_ref().filter(|_| show_raw_agent_reasoning) {
                for c in items {
                    let text = match c {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => text,
                    };
                    events.push(EventMsg::AgentReasoningRawContent(
                        AgentReasoningRawContentEvent { text: text.clone() },
                    ));
                }
            }
            events
        }

        ResponseItem::WebSearchCall { id, action, .. } => match action {
            WebSearchAction::Search { query } => {
                let call_id = id.clone().unwrap_or_else(|| "".to_string());
                vec![EventMsg::WebSearchEnd(WebSearchEndEvent {
                    call_id,
                    query: query.clone(),
                })]
            }
            WebSearchAction::Other => Vec::new(),
        },

        // Variants that require side effects are handled by higher layers and do not emit events here.
        ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::Other => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::map_response_item_to_event_messages;
    use crate::protocol::EventMsg;
    use crate::protocol::InputMessageKind;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn maps_user_message_with_text_and_two_images() {
        let img1 = "https://example.com/one.png".to_string();
        let img2 = "https://example.com/two.jpg".to_string();

        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "Hello world".to_string(),
                },
                ContentItem::InputImage {
                    image_url: img1.clone(),
                },
                ContentItem::InputImage {
                    image_url: img2.clone(),
                },
            ],
        };

        let events = map_response_item_to_event_messages(&item, false);
        assert_eq!(events.len(), 1, "expected a single user message event");

        match &events[0] {
            EventMsg::UserMessage(user) => {
                assert_eq!(user.message, "Hello world");
                assert!(matches!(user.kind, Some(InputMessageKind::Plain)));
                assert_eq!(user.images, Some(vec![img1, img2]));
            }
            other => panic!("expected UserMessage, got {other:?}"),
        }
    }
}
