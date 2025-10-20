use crate::protocol::EventMsg;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::user_input::UserInput;
use tracing::warn;

fn is_session_prefix(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with("<environment_context>") || lowered.starts_with("<user_instructions>")
}

fn parse_user_message(message: &[ContentItem]) -> Option<UserMessageItem> {
    let mut content: Vec<UserInput> = Vec::new();

    for content_item in message.iter() {
        match content_item {
            ContentItem::InputText { text } => {
                if is_session_prefix(text) {
                    return None;
                }
                content.push(UserInput::Text { text: text.clone() });
            }
            ContentItem::InputImage { image_url } => {
                content.push(UserInput::Image {
                    image_url: image_url.clone(),
                });
            }
            ContentItem::OutputText { text } => {
                if is_session_prefix(text) {
                    return None;
                }
                warn!("Output text in user message: {}", text);
            }
        }
    }

    Some(UserMessageItem::new(&content))
}

fn parse_agent_message(message: &[ContentItem]) -> AgentMessageItem {
    let mut output: String = String::new();
    for content_item in message.iter() {
        match content_item {
            ContentItem::OutputText { text } => {
                output = text.to_string();
            }
            _ => {
                warn!(
                    "Unexpected content item in agent message: {:?}",
                    content_item
                );
            }
        }
    }
    AgentMessageItem::new(&[AgentMessageContent::Text { text: output }])
}

pub fn parse_turn_item(item: &ResponseItem) -> Option<TurnItem> {
    match item {
        ResponseItem::Message { role, content, .. } => match role.as_str() {
            "user" => parse_user_message(content).map(TurnItem::UserMessage),
            "assistant" => Some(TurnItem::AgentMessage(parse_agent_message(content))),
            "system" => None,
            _ => None,
        },
        ResponseItem::Reasoning {
            id,
            summary,
            content,
            encrypted_content,
        } => {
            let summary_text = summary
                .iter()
                .map(|entry| match entry {
                    ReasoningItemReasoningSummary::SummaryText { text } => text.clone(),
                })
                .collect();
            let raw_content = content
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|entry| match entry {
                    ReasoningItemContent::ReasoningText { text }
                    | ReasoningItemContent::Text { text } => text,
                })
                .collect();
            Some(TurnItem::Reasoning(ReasoningItem {
                id: id.clone(),
                summary_text,
                raw_content,
                encrypted_content: encrypted_content.clone(),
            }))
        }
        ResponseItem::WebSearchCall {
            id,
            action: WebSearchAction::Search { query },
            ..
        } => Some(TurnItem::WebSearch(WebSearchItem {
            id: id.clone().unwrap_or_default(),
            query: query.clone(),
        })),
        _ => None,
    }
}

/// Convert a `ResponseItem` into zero or more `EventMsg` values that the UI can render.
///
/// When `show_raw_agent_reasoning` is false, raw reasoning content events are omitted.
pub(crate) fn map_response_item_to_event_messages(
    item: &ResponseItem,
    show_raw_agent_reasoning: bool,
) -> Vec<EventMsg> {
    if let Some(turn_item) = parse_turn_item(item) {
        return turn_item.legacy_events(show_raw_agent_reasoning);
    }

    // Variants that require side effects are handled by higher layers and do not emit events here.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::map_response_item_to_event_messages;
    use crate::protocol::EventMsg;
    use crate::protocol::WebSearchEndEvent;

    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::models::WebSearchAction;
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
                assert_eq!(user.images, Some(vec![img1, img2]));
            }
            other => panic!("expected UserMessage, got {other:?}"),
        }
    }

    #[test]
    fn maps_reasoning_summary_without_raw_content() {
        let item = ResponseItem::Reasoning {
            id: "reasoning_1".to_string(),
            summary: vec![
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 1".to_string(),
                },
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 2".to_string(),
                },
            ],
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: "raw details".to_string(),
            }]),
            encrypted_content: None,
        };

        let events = map_response_item_to_event_messages(&item, false);

        assert_eq!(events.len(), 2, "expected only reasoning summaries");
        assert!(
            events
                .iter()
                .all(|event| matches!(event, EventMsg::AgentReasoning(_)))
        );
    }

    #[test]
    fn maps_reasoning_including_raw_content_when_enabled() {
        let item = ResponseItem::Reasoning {
            id: "reasoning_2".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Summarized step".to_string(),
            }],
            content: Some(vec![
                ReasoningItemContent::ReasoningText {
                    text: "raw step".to_string(),
                },
                ReasoningItemContent::Text {
                    text: "final thought".to_string(),
                },
            ]),
            encrypted_content: None,
        };

        let events = map_response_item_to_event_messages(&item, true);

        assert_eq!(
            events.len(),
            3,
            "expected summary and raw reasoning content events"
        );
        assert!(matches!(events[0], EventMsg::AgentReasoning(_)));
        assert!(matches!(events[1], EventMsg::AgentReasoningRawContent(_)));
        assert!(matches!(events[2], EventMsg::AgentReasoningRawContent(_)));
    }

    #[test]
    fn maps_web_search_call() {
        let item = ResponseItem::WebSearchCall {
            id: Some("ws_1".to_string()),
            status: Some("completed".to_string()),
            action: WebSearchAction::Search {
                query: "weather".to_string(),
            },
        };

        let events = map_response_item_to_event_messages(&item, false);
        assert_eq!(events.len(), 1, "expected a single web search event");

        match &events[0] {
            EventMsg::WebSearchEnd(WebSearchEndEvent { call_id, query }) => {
                assert_eq!(call_id, "ws_1");
                assert_eq!(query, "weather");
            }
            other => panic!("expected WebSearchEnd, got {other:?}"),
        }
    }
}
