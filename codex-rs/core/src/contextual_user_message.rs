use codex_protocol::items::HookPromptItem;
use codex_protocol::items::parse_hook_prompt_fragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;

pub(crate) const AGENTS_MD_START_MARKER: &str = "# AGENTS.md instructions for ";
pub(crate) const AGENTS_MD_END_MARKER: &str = "</INSTRUCTIONS>";
pub(crate) const SKILL_OPEN_TAG: &str = "<skill>";
pub(crate) const SKILL_CLOSE_TAG: &str = "</skill>";
pub(crate) const USER_SHELL_COMMAND_OPEN_TAG: &str = "<user_shell_command>";
pub(crate) const USER_SHELL_COMMAND_CLOSE_TAG: &str = "</user_shell_command>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const TURN_ABORTED_CLOSE_TAG: &str = "</turn_aborted>";
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str = "<subagent_notification>";
pub(crate) const SUBAGENT_NOTIFICATION_CLOSE_TAG: &str = "</subagent_notification>";

#[derive(Clone, Copy)]
pub(crate) struct ContextualUserFragmentDefinition {
    start_marker: &'static str,
    end_marker: &'static str,
}

impl ContextualUserFragmentDefinition {
    pub(crate) const fn new(start_marker: &'static str, end_marker: &'static str) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }

    pub(crate) fn matches_text(&self, text: &str) -> bool {
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..self.start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(self.end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.end_marker));
        starts_with_marker && ends_with_marker
    }

    pub(crate) const fn start_marker(&self) -> &'static str {
        self.start_marker
    }

    pub(crate) const fn end_marker(&self) -> &'static str {
        self.end_marker
    }

    pub(crate) fn wrap(&self, body: String) -> String {
        format!("{}\n{}\n{}", self.start_marker, body, self.end_marker)
    }

    pub(crate) fn into_message(self, text: String) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text }],
            metadata: None,
            end_turn: None,
            phase: None,
        }
    }
}

pub(crate) const AGENTS_MD_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(AGENTS_MD_START_MARKER, AGENTS_MD_END_MARKER);
pub(crate) const ENVIRONMENT_CONTEXT_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(
        ENVIRONMENT_CONTEXT_OPEN_TAG,
        ENVIRONMENT_CONTEXT_CLOSE_TAG,
    );
pub(crate) const SKILL_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(SKILL_OPEN_TAG, SKILL_CLOSE_TAG);
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
