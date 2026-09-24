use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use super::AdditionalContextUserFragment;
use super::AgentMessageBoardNotification;
use super::ContextualUserFragment;
use super::InternalModelContextFragment;
use super::LegacyApplyPatchExecCommandWarning;
use super::LegacyModelMismatchWarning;
use super::LegacyUnifiedExecProcessLimitWarning;
use super::RecommendedPluginsInstructions;
use super::SubagentNotification;
use super::TurnAborted;
use super::UserGoalUpdate;
use super::UserInstructions;
use super::UserShellCommand;
use super::world_state::EnvironmentsState;

const CONTEXTUAL_USER_FRAGMENT_MATCHERS: &[fn(&str) -> bool] = &[
    UserInstructions::matches_text,
    EnvironmentsState::matches_text,
    AdditionalContextUserFragment::matches_text,
    AgentMessageBoardNotification::matches_text,
    codex_skills_extension::is_skill_prompt_fragment,
    UserShellCommand::matches_text,
    TurnAborted::matches_text,
    SubagentNotification::matches_text,
    InternalModelContextFragment::matches_text,
    // compatibility for user-role recommendation messages in existing rollouts
    RecommendedPluginsInstructions::matches_text,
    LegacyUnifiedExecProcessLimitWarning::matches_text,
    LegacyApplyPatchExecCommandWarning::matches_text,
    LegacyModelMismatchWarning::matches_text,
];

/// Hidden runtime context is not user authorization. Explicit user goal edits are.
pub(crate) fn is_guardian_context_message(item: &ResponseItem) -> bool {
    matches!(item, ResponseItem::Message { role, content, .. }
        if role == "user"
            && content.iter().any(is_contextual_user_fragment)
            && UserGoalUpdate::message_text(item).is_none())
}

/// Uses host annotations rather than text markers to identify user authorization changes.
pub(crate) fn is_user_authorization_message(item: &ResponseItem) -> bool {
    let ResponseItem::Message {
        role,
        content,
        internal_chat_message_metadata_passthrough,
        ..
    } = item
    else {
        return false;
    };
    role == "user"
        && internal_chat_message_metadata_passthrough
            .as_ref()
            .and_then(|metadata| metadata.content_item_kinds.as_ref())
            .is_none_or(|kinds| {
                // Unknown, incomplete, and legacy messages remain conservative.
                kinds.is_empty()
                    || kinds.len() != content.len()
                    || kinds.iter().any(|kind| {
                        kind.0.starts_with("user.")
                            || matches!(
                                kind.0.as_str(),
                                "" | "unknown"
                                    // Media preparation can replace real user input.
                                    | "images.preparation_error"
                                    | "images.unsupported"
                                    | "audio.unsupported"
                            )
                    })
            })
}

fn is_standard_contextual_user_text(text: &str) -> bool {
    CONTEXTUAL_USER_FRAGMENT_MATCHERS
        .iter()
        .any(|matches_text| matches_text(text))
}

pub(crate) fn is_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };
    parse_hook_prompt_fragment(text).is_some() || is_standard_contextual_user_text(text)
}

pub(crate) fn parse_visible_hook_prompt_message(
    id: Option<&str>,
    content: &[ContentItem],
) -> Option<HookPromptItem> {
    let mut fragments = Vec::new();

    for content_item in content {
        let ContentItem::InputText { text } = content_item else {
            return None;
        };
        if let Some(fragment) = parse_hook_prompt_fragment(text) {
            fragments.push(fragment);
            continue;
        }
        if is_standard_contextual_user_text(text) {
            continue;
        }
        return None;
    }

    if fragments.is_empty() {
        return None;
    }

    Some(HookPromptItem::from_fragments(id, fragments))
}

#[cfg(test)]
#[path = "contextual_user_message_tests.rs"]
mod tests;
