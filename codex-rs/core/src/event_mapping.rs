use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::models::is_image_close_tag_text;
use codex_protocol::models::is_image_open_tag_text;
use codex_protocol::models::is_local_image_close_tag_text;
use codex_protocol::models::is_local_image_open_tag_text;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;
use codex_protocol::user_input::UserInput;
use tracing::warn;
use uuid::Uuid;

use crate::context::is_contextual_user_fragment;
use crate::context::parse_visible_hook_prompt_message;
use crate::web_search::web_search_action_detail;

const CONTEXTUAL_DEVELOPER_PREFIXES: &[&str] = &[
    "<permissions instructions>",
    "<model_switch>",
    COLLABORATION_MODE_OPEN_TAG,
    REALTIME_CONVERSATION_OPEN_TAG,
    "<personality_spec>",
];

pub(crate) fn is_contextual_user_message_content(message: &[ContentItem]) -> bool {
    message.iter().any(is_contextual_user_fragment)
}

/// Returns true when a developer message contains any rollback-trimmable contextual fragment.
///
/// `build_initial_context` can bundle these fragments together with persistent developer text in a
/// single developer message, so callers that care about invalidating a stored reference baseline
/// should pair this with `has_non_contextual_dev_message_content`.
pub(crate) fn is_contextual_dev_message_content(message: &[ContentItem]) -> bool {
    message.iter().any(is_contextual_dev_fragment)
}

/// Returns true when a developer message contains any fragment that is not part of the
/// rollback-trimmable contextual prefix set.
pub(crate) fn has_non_contextual_dev_message_content(message: &[ContentItem]) -> bool {
    message
        .iter()
        .any(|content_item| !is_contextual_dev_fragment(content_item))
}

fn is_contextual_dev_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };

    let trimmed = text.trim_start();
    CONTEXTUAL_DEVELOPER_PREFIXES.iter().any(|prefix| {
        trimmed
            .get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
    })
}

fn parse_user_message(message: &[ContentItem]) -> Option<UserMessageItem> {
    if is_contextual_user_message_content(message) {
        return None;
    }

    let mut content: Vec<UserInput> = Vec::new();

    for (idx, content_item) in message.iter().enumerate() {
        match content_item {
            ContentItem::InputText { text } => {
                if (is_local_image_open_tag_text(text) || is_image_open_tag_text(text))
                    && (matches!(message.get(idx + 1), Some(ContentItem::InputImage { .. })))
                    || (idx > 0
                        && (is_local_image_close_tag_text(text) || is_image_close_tag_text(text))
                        && matches!(message.get(idx - 1), Some(ContentItem::InputImage { .. })))
                {
                    continue;
                }
                content.push(UserInput::Text {
                    text: text.clone(),
                    // Model input content does not carry UI element ranges.
                    text_elements: Vec::new(),
                });
            }
            ContentItem::InputImage { image_url, .. } => {
                content.push(UserInput::Image {
                    image_url: image_url.clone(),
                });
            }
            ContentItem::OutputText { text } => {
                warn!("Output text in user message: {}", text);
            }
        }
    }

    Some(UserMessageItem::new(&content))
}

fn parse_agent_message(
    id: Option<&String>,
    message: &[ContentItem],
    phase: Option<MessagePhase>,
) -> AgentMessageItem {
    let mut content: Vec<AgentMessageContent> = Vec::new();
    for content_item in message.iter() {
        match content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                content.push(AgentMessageContent::Text { text: text.clone() });
            }
            _ => {
                warn!(
                    "Unexpected content item in agent message: {:?}",
                    content_item
                );
            }
        }
    }
    let id = id.cloned().unwrap_or_else(|| Uuid::new_v4().to_string());
    AgentMessageItem {
        id,
        content,
        phase,
        memory_citation: None,
    }
}

pub fn parse_turn_item(item: &ResponseItem) -> Option<TurnItem> {
    match item {
        ResponseItem::Message {
            role,
            content,
            id,
            phase,
            ..
        } => match role.as_str() {
            "user" => parse_visible_hook_prompt_message(id.as_ref(), content)
                .map(TurnItem::HookPrompt)
                .or_else(|| parse_user_message(content).map(TurnItem::UserMessage)),
            "assistant" => Some(TurnItem::AgentMessage(parse_agent_message(
                id.as_ref(),
                content,
                phase.clone(),
            ))),
            "system" => None,
            _ => None,
        },
        ResponseItem::Reasoning {
            id,
            summary,
            content,
            ..
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
            }))
        }
        ResponseItem::WebSearchCall { id, action, .. } => {
            let (action, query) = match action {
                Some(action) => (action.clone(), web_search_action_detail(action)),
                None => (WebSearchAction::Other, String::new()),
            };
            Some(TurnItem::WebSearch(WebSearchItem {
                id: id.clone().unwrap_or_default(),
                query,
                action,
            }))
        }
        ResponseItem::ImageGenerationCall {
            id,
            status,
            revised_prompt,
            result,
        } => Some(TurnItem::ImageGeneration(
            codex_protocol::items::ImageGenerationItem {
                id: id.clone(),
                status: status.clone(),
                revised_prompt: revised_prompt.clone(),
                result: result.clone(),
                saved_path: None,
            },
        )),
        _ => None,
    }
}

#[cfg(test)]
#[path = "event_mapping_tests.rs"]
mod tests;
