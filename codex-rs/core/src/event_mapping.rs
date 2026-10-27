use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::models::is_audio_close_tag_text;
use codex_protocol::models::is_audio_open_tag_text;
use codex_protocol::models::is_image_close_tag_text;
use codex_protocol::models::is_image_open_tag_text;
use codex_protocol::models::is_local_audio_close_tag_text;
use codex_protocol::models::is_local_audio_open_tag_text;
use codex_protocol::models::is_local_image_close_tag_text;
use codex_protocol::models::is_local_image_open_tag_text;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_OPEN_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_OPEN_TAG;
use codex_protocol::protocol::ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::TOOLS_OPEN_TAG;
use codex_protocol::user_input::UserInput;
use tracing::warn;
use uuid::Uuid;

use crate::context::APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX;
use crate::context::is_contextual_user_fragment;
use crate::context::parse_visible_hook_prompt_message;
use crate::web_search::web_search_action_detail;

const CONTEXTUAL_DEVELOPER_PREFIXES: &[&str] = &[
    "<permissions instructions>",
    APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX,
    "<model_switch>",
    "<managed_developer_instructions>",
    "<persistent_mode>",
    APPS_INSTRUCTIONS_OPEN_TAG,
    COLLABORATION_MODE_OPEN_TAG,
    "<multi_agent_role>",
    MULTI_AGENT_MODE_OPEN_TAG,
    ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG,
    "<git_attribution>",
    PLUGINS_INSTRUCTIONS_OPEN_TAG,
    REALTIME_CONVERSATION_OPEN_TAG,
    SKILLS_INSTRUCTIONS_OPEN_TAG,
    TOOLS_OPEN_TAG,
    "<personality_spec>",
    // Keep recognizing token-budget wrappers persisted by older versions.
    "<token_budget>",
    CONTEXT_WINDOW_OPEN_TAG,
    CONTEXT_WINDOW_GUIDANCE_OPEN_TAG,
    "<rollout_budget>",
];

pub(crate) fn is_contextual_user_message_content(message: &[ContentItem]) -> bool {
    message.iter().any(is_contextual_user_fragment)
}

/// Classifies each fragment for rollback. A catalog heading can change while its
/// harness-owned kind stays stable. Older messages only carry the literal text.
pub(crate) fn contextual_dev_fragments<'a>(
    message: &'a [ContentItem],
    kinds: &'a [ContentItemKind],
) -> impl Iterator<Item = bool> + 'a {
    message.iter().enumerate().map(|(index, item)| {
        kinds
            .get(index)
            .is_some_and(|kind| kind.0 == crate::context::SAVED_PREFIX_KIND)
            || is_contextual_dev_fragment(item)
    })
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

/// User-input parts with their original indices. Media wrapper labels are not
/// user text; retain the indices so history edits can carry the right annotations.
pub(crate) fn user_input_parts(
    message: &[ContentItem],
) -> impl Iterator<Item = (usize, &ContentItem)> {
    message
        .iter()
        .enumerate()
        .filter(|(idx, content_item)| match content_item {
            ContentItem::InputText { text } => {
                let is_image_label = ((is_local_image_open_tag_text(text)
                    || is_image_open_tag_text(text))
                    && matches!(message.get(idx + 1), Some(ContentItem::InputImage { .. })))
                    || (*idx > 0
                        && (is_local_image_close_tag_text(text) || is_image_close_tag_text(text))
                        && matches!(message.get(idx - 1), Some(ContentItem::InputImage { .. })));
                let is_audio_label = ((is_local_audio_open_tag_text(text)
                    || is_audio_open_tag_text(text))
                    && matches!(message.get(idx + 1), Some(ContentItem::InputAudio { .. })))
                    || (*idx > 0
                        && (is_local_audio_close_tag_text(text) || is_audio_close_tag_text(text))
                        && matches!(message.get(idx - 1), Some(ContentItem::InputAudio { .. })));
                !is_image_label && !is_audio_label
            }
            ContentItem::InputImage { .. }
            | ContentItem::InputAudio { .. }
            | ContentItem::OutputText { .. } => true,
        })
}

fn parse_user_message(message: &[ContentItem]) -> Option<UserMessageItem> {
    if is_contextual_user_message_content(message) {
        return None;
    }

    let mut content: Vec<UserInput> = Vec::new();

    for (_, content_item) in user_input_parts(message) {
        match content_item {
            ContentItem::InputText { text } => {
                content.push(UserInput::Text {
                    text: text.clone(),
                    // Model input content does not carry UI element ranges.
                    text_elements: Vec::new(),
                });
            }
            ContentItem::InputImage { image, detail } => {
                content.push(UserInput::Image {
                    image: image.clone(),
                    detail: *detail,
                });
            }
            ContentItem::InputAudio { audio_url } => {
                content.push(UserInput::Audio {
                    audio_url: audio_url.clone(),
                });
            }
            ContentItem::OutputText { text, .. } => {
                warn!("Output text in user message: {}", text);
            }
        }
    }

    Some(UserMessageItem::new(&content))
}

fn parse_agent_message(
    id: Option<&str>,
    message: &[ContentItem],
    phase: Option<MessagePhase>,
) -> AgentMessageItem {
    let mut content: Vec<AgentMessageContent> = Vec::new();
    for content_item in message.iter() {
        match content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
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
    let id = id
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    AgentMessageItem {
        id,
        content,
        phase,
        memory_citation: None,
        delivery: None,
        questions: None,
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
            "user" => parse_visible_hook_prompt_message(id.as_deref(), content)
                .map(TurnItem::HookPrompt)
                .or_else(|| parse_user_message(content).map(TurnItem::UserMessage)),
            "assistant" => Some(TurnItem::AgentMessage(parse_agent_message(
                id.as_deref(),
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
                id: id.as_deref().unwrap_or_default().to_string(),
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
                id: id.as_deref().unwrap_or_default().to_string(),
                query,
                action,
                results: None,
            }))
        }
        ResponseItem::ImageGenerationCall {
            id,
            status,
            revised_prompt,
            result,
            ..
        } => Some(TurnItem::ImageGeneration(
            codex_protocol::items::ImageGenerationItem {
                id: id.as_deref()?.to_string(),
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
