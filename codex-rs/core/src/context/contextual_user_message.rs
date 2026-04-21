use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;

use super::EnvironmentContext;
use super::FragmentRegistration;
use super::FragmentRegistrationProxy;
use super::SkillInstructions;
use super::SubagentNotification;
use super::TurnAborted;
use super::UserInstructions;
use super::UserShellCommand;

static USER_INSTRUCTIONS_REGISTRATION: FragmentRegistrationProxy<UserInstructions> =
    FragmentRegistrationProxy::new();
static ENVIRONMENT_CONTEXT_REGISTRATION: FragmentRegistrationProxy<EnvironmentContext> =
    FragmentRegistrationProxy::new();
static SKILL_INSTRUCTIONS_REGISTRATION: FragmentRegistrationProxy<SkillInstructions> =
    FragmentRegistrationProxy::new();
static USER_SHELL_COMMAND_REGISTRATION: FragmentRegistrationProxy<UserShellCommand> =
    FragmentRegistrationProxy::new();
static TURN_ABORTED_REGISTRATION: FragmentRegistrationProxy<TurnAborted> =
    FragmentRegistrationProxy::new();
static SUBAGENT_NOTIFICATION_REGISTRATION: FragmentRegistrationProxy<SubagentNotification> =
    FragmentRegistrationProxy::new();

static CONTEXTUAL_USER_FRAGMENTS: &[&dyn FragmentRegistration] = &[
    &USER_INSTRUCTIONS_REGISTRATION,
    &ENVIRONMENT_CONTEXT_REGISTRATION,
    &SKILL_INSTRUCTIONS_REGISTRATION,
    &USER_SHELL_COMMAND_REGISTRATION,
    &TURN_ABORTED_REGISTRATION,
    &SUBAGENT_NOTIFICATION_REGISTRATION,
];

static MEMORY_EXCLUDED_CONTEXTUAL_USER_FRAGMENTS: &[&dyn FragmentRegistration] = &[
    &USER_INSTRUCTIONS_REGISTRATION,
    &SKILL_INSTRUCTIONS_REGISTRATION,
];

fn is_standard_contextual_user_text(text: &str) -> bool {
    CONTEXTUAL_USER_FRAGMENTS
        .iter()
        .any(|fragment| fragment.matches_text(text))
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
    MEMORY_EXCLUDED_CONTEXTUAL_USER_FRAGMENTS
        .iter()
        .any(|fragment| fragment.matches_text(text))
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
