use codex_instructions::AGENTS_MD_FRAGMENT;
use codex_instructions::ContextualUserFragmentDefinition;
use codex_instructions::SKILL_FRAGMENT;
use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;

pub(crate) const USER_SHELL_COMMAND_OPEN_TAG: &str = "<user_shell_command>";
pub(crate) const USER_SHELL_COMMAND_CLOSE_TAG: &str = "</user_shell_command>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const TURN_ABORTED_CLOSE_TAG: &str = "</turn_aborted>";
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str = "<subagent_notification>";
pub(crate) const SUBAGENT_NOTIFICATION_CLOSE_TAG: &str = "</subagent_notification>";

pub(crate) const ENVIRONMENT_CONTEXT_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(
        ENVIRONMENT_CONTEXT_OPEN_TAG,
        ENVIRONMENT_CONTEXT_CLOSE_TAG,
    );
pub(crate) const USER_SHELL_COMMAND_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(
        USER_SHELL_COMMAND_OPEN_TAG,
        USER_SHELL_COMMAND_CLOSE_TAG,
    );
pub(crate) const TURN_ABORTED_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(TURN_ABORTED_OPEN_TAG, TURN_ABORTED_CLOSE_TAG);
pub(crate) const SUBAGENT_NOTIFICATION_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(
        SUBAGENT_NOTIFICATION_OPEN_TAG,
        SUBAGENT_NOTIFICATION_CLOSE_TAG,
    );

const CONTEXTUAL_USER_FRAGMENTS: &[ContextualUserFragmentDefinition] = &[
    AGENTS_MD_FRAGMENT,
    ENVIRONMENT_CONTEXT_FRAGMENT,
    SKILL_FRAGMENT,
    USER_SHELL_COMMAND_FRAGMENT,
    TURN_ABORTED_FRAGMENT,
    SUBAGENT_NOTIFICATION_FRAGMENT,
];

fn is_standard_contextual_user_text(text: &str) -> bool {
    CONTEXTUAL_USER_FRAGMENTS
        .iter()
        .any(|definition| definition.matches_text(text))
}

/// Returns whether a contextual user fragment should be omitted from memory
/// stage-1 inputs.
///
/// We exclude injected `AGENTS.md` instructions and skill payloads because
/// they are prompt scaffolding rather than conversation content, so they do
/// not improve the resulting memory. We keep environment context and
/// subagent notifications because they can carry useful execution context or
/// subtask outcomes that should remain visible to memory generation.
pub(crate) fn is_memory_excluded_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };
    AGENTS_MD_FRAGMENT.matches_text(text) || SKILL_FRAGMENT.matches_text(text)
}

pub(crate) fn is_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };
    parse_hook_prompt_fragment(text).is_some() || is_standard_contextual_user_text(text)
}

pub(crate) fn parse_visible_hook_prompt_message(
    id: Option<&String>,
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
