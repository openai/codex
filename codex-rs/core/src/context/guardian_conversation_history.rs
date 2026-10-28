//! Instructions for retrieving historical authorization in an opted-in Guardian review.
//! This optionally overridden fragment is installed outside the parent transcript.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

pub(crate) struct GuardianConversationHistory<'a> {
    pub prompt: Option<&'a str>,
}

impl ContextualUserFragment for GuardianConversationHistory<'_> {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.conversation_history".to_owned())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(
            self.prompt
                .unwrap_or(codex_guardian_reviewer::GUARDIAN_CONVERSATION_HISTORY_PROMPT)
                .to_owned(),
        )
    }
}
