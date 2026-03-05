//! Shared model-visible context abstractions.
//!
//! Use this path for any injected prompt context, whether it renders in the
//! developer envelope or the contextual-user envelope.
//!
//! Contextual-user fragments must provide stable markers so history parsing can
//! distinguish them from real user intent. Developer fragments do not need
//! markers because they are already separable by role.

use crate::codex::TurnContext;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_protocol::protocol::TurnContextItem;

pub(crate) const AGENTS_MD_START_MARKER: &str = "# AGENTS.md instructions for ";
pub(crate) const AGENTS_MD_END_MARKER: &str = "</INSTRUCTIONS>";
pub(crate) const SKILL_OPEN_TAG: &str = "<skill>";
pub(crate) const SKILL_CLOSE_TAG: &str = "</skill>";
pub(crate) const USER_SHELL_COMMAND_OPEN_TAG: &str = "<user_shell_command>";
pub(crate) const USER_SHELL_COMMAND_CLOSE_TAG: &str = "</user_shell_command>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const TURN_ABORTED_CLOSE_TAG: &str = "</turn_aborted>";
pub(crate) const PLUGINS_OPEN_TAG: &str = "<plugins>";
pub(crate) const PLUGINS_CLOSE_TAG: &str = "</plugins>";
pub(crate) const SUBAGENTS_OPEN_TAG: &str = "<subagents>";
pub(crate) const SUBAGENTS_CLOSE_TAG: &str = "</subagents>";
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str = "<subagent_notification>";
pub(crate) const SUBAGENT_NOTIFICATION_CLOSE_TAG: &str = "</subagent_notification>";

pub(crate) trait ModelVisibleContextEnvelopeKind {
    const RESPONSE_ROLE: &'static str;
}

pub(crate) struct DeveloperEnvelopeKind;

impl ModelVisibleContextEnvelopeKind for DeveloperEnvelopeKind {
    const RESPONSE_ROLE: &'static str = "developer";
}

pub(crate) struct ContextualUserEnvelopeKind;

impl ModelVisibleContextEnvelopeKind for ContextualUserEnvelopeKind {
    const RESPONSE_ROLE: &'static str = "user";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModelVisibleContextEnvelope {
    start_marker: Option<&'static str>,
    end_marker: Option<&'static str>,
}

impl ModelVisibleContextEnvelope {
    pub(crate) const fn untagged() -> Self {
        Self {
            start_marker: None,
            end_marker: None,
        }
    }

    pub(crate) const fn contextual_user(
        start_marker: &'static str,
        end_marker: &'static str,
    ) -> Self {
        Self {
            start_marker: Some(start_marker),
            end_marker: Some(end_marker),
        }
    }

    pub(crate) fn matches_text(&self, text: &str) -> bool {
        let (Some(start_marker), Some(end_marker)) = (self.start_marker, self.end_marker) else {
            return false;
        };
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
        starts_with_marker && ends_with_marker
    }

    pub(crate) fn start_marker(&self) -> &'static str {
        match self.start_marker {
            Some(start_marker) => start_marker,
            None => panic!("model-visible fragment has no start marker"),
        }
    }

    pub(crate) fn end_marker(&self) -> &'static str {
        match self.end_marker {
            Some(end_marker) => end_marker,
            None => panic!("model-visible fragment has no end marker"),
        }
    }

    pub(crate) fn wrap_body(&self, body: String) -> String {
        format!("{}\n{}\n{}", self.start_marker(), body, self.end_marker())
    }

    pub(crate) fn into_content_item(self, text: String) -> ContentItem {
        ContentItem::InputText { text }
    }

    pub(crate) fn into_message<K: ModelVisibleContextEnvelopeKind>(
        self,
        text: String,
    ) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: K::RESPONSE_ROLE.to_string(),
            content: vec![self.into_content_item(text)],
            end_turn: None,
            phase: None,
        }
    }

    pub(crate) fn into_response_input_item<K: ModelVisibleContextEnvelopeKind>(
        self,
        text: String,
    ) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: K::RESPONSE_ROLE.to_string(),
            content: vec![self.into_content_item(text)],
        }
    }
}

/// Implement this for any model-visible prompt fragment, regardless of which
/// envelope it renders into.
pub(crate) trait ModelVisibleContextFragment {
    type Kind: ModelVisibleContextEnvelopeKind;

    fn spec(&self) -> ModelVisibleContextEnvelope;

    fn render_text(&self) -> String;

    fn into_content_item(self) -> ContentItem
    where
        Self: Sized,
    {
        self.spec().into_content_item(self.render_text())
    }

    fn into_response_input_item(self) -> ResponseInputItem
    where
        Self: Sized,
    {
        self.spec()
            .into_response_input_item::<Self::Kind>(self.render_text())
    }

    fn into_message(self) -> ResponseItem
    where
        Self: Sized,
    {
        self.spec().into_message::<Self::Kind>(self.render_text())
    }
}

/// Implement this for fragments that are built from current/persisted turn
/// state rather than one-off runtime events.
pub(crate) trait TurnContextFragment: ModelVisibleContextFragment + Sized {
    fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Option<Self>;

    fn from_turn_context_item(turn_context_item: &TurnContextItem, shell: &Shell) -> Option<Self>;

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        shell: &Shell,
    ) -> Option<Self>;
}

pub(crate) const DEVELOPER_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::untagged();
pub(crate) const AGENTS_MD_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(AGENTS_MD_START_MARKER, AGENTS_MD_END_MARKER);
pub(crate) const ENVIRONMENT_CONTEXT_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(
        ENVIRONMENT_CONTEXT_OPEN_TAG,
        ENVIRONMENT_CONTEXT_CLOSE_TAG,
    );
pub(crate) const SKILL_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(SKILL_OPEN_TAG, SKILL_CLOSE_TAG);
pub(crate) const USER_SHELL_COMMAND_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(
        USER_SHELL_COMMAND_OPEN_TAG,
        USER_SHELL_COMMAND_CLOSE_TAG,
    );
pub(crate) const TURN_ABORTED_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(TURN_ABORTED_OPEN_TAG, TURN_ABORTED_CLOSE_TAG);
pub(crate) const PLUGINS_FRAGMENT: ModelVisibleContextEnvelope =
    ModelVisibleContextEnvelope::contextual_user(PLUGINS_OPEN_TAG, PLUGINS_CLOSE_TAG);

const CONTEXTUAL_USER_FRAGMENTS: &[ModelVisibleContextEnvelope] = &[
    AGENTS_MD_FRAGMENT,
    ENVIRONMENT_CONTEXT_FRAGMENT,
    SKILL_FRAGMENT,
    USER_SHELL_COMMAND_FRAGMENT,
    TURN_ABORTED_FRAGMENT,
    PLUGINS_FRAGMENT,
];

pub(crate) fn is_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };
    CONTEXTUAL_USER_FRAGMENTS
        .iter()
        .any(|definition| definition.matches_text(text))
}

impl ModelVisibleContextFragment for DeveloperInstructions {
    type Kind = DeveloperEnvelopeKind;

    fn spec(&self) -> ModelVisibleContextEnvelope {
        DEVELOPER_FRAGMENT
    }

    fn render_text(&self) -> String {
        self.clone().into_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_environment_context_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
        }));
    }

    #[test]
    fn detects_agents_instructions_fragment() {
        assert!(is_contextual_user_fragment(&ContentItem::InputText {
            text: "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
                .to_string(),
        }));
    }

    #[test]
    fn ignores_regular_user_text() {
        assert!(!is_contextual_user_fragment(&ContentItem::InputText {
            text: "hello".to_string(),
        }));
    }

    #[test]
    fn developer_spec_does_not_match_contextual_user_text() {
        assert!(!DEVELOPER_FRAGMENT.matches_text("<permissions instructions>body"));
    }
}
