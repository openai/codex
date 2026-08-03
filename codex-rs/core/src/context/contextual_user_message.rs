use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;

use super::AdditionalContextUserFragment;
use super::ContextualUserFragment;
use super::InternalModelContextFragment;
use super::LegacyApplyPatchExecCommandWarning;
use super::LegacyModelMismatchWarning;
use super::LegacyUnifiedExecProcessLimitWarning;
use super::RecommendedPluginsInstructions;
use super::SkillInstructions;
use super::SubagentNotification;
use super::TurnAborted;
use super::UserInstructions;
use super::UserShellCommand;
use super::world_state::EnvironmentsState;

const CONTEXTUAL_USER_FRAGMENT_MATCHERS: &[fn(&str) -> bool] = &[
    UserInstructions::matches_text,
    EnvironmentsState::matches_text,
    AdditionalContextUserFragment::matches_text,
    SkillInstructions::matches_text,
    UserShellCommand::matches_text,
    TurnAborted::matches_text,
    SubagentNotification::matches_text,
    InternalModelContextFragment::matches_text,
    RecommendedPluginsInstructions::matches_text,
    LegacyUnifiedExecProcessLimitWarning::matches_text,
    LegacyApplyPatchExecCommandWarning::matches_text,
    LegacyModelMismatchWarning::matches_text,
];

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
