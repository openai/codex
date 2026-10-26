//! Marks bounded assistant evidence used to interpret user replies in Guardian reviews.
//! Assistant text remains conversational context and never grants authorization.

use codex_history::RetainedUserMessage;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

use super::ContextualUserFragment;

struct GuardianAssistantContext<'a>(&'a str);

impl ContextualUserFragment for GuardianAssistantContext<'_> {
    fn role(&self) -> &'static str {
        "assistant"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.assistant_context".to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.0)
    }
}

/// Renders original assistant context through its typed fragment after checking the
/// whole role-labeled message against the retained evidence budget.
pub fn render_retained_assistant_context(message: &RetainedUserMessage) -> Option<String> {
    codex_guardian_context::retained_assistant_message(message)?;
    render_original_assistant_context(message)
}

/// Preserves the host fragment boundary while deferring sync size admission.
pub(crate) fn render_original_assistant_context(message: &RetainedUserMessage) -> Option<String> {
    if !message.complete {
        return None;
    }
    Some(
        GuardianAssistantContext(&message.text)
            .render()
            .into_text()
            .unwrap_or_default(),
    )
}
