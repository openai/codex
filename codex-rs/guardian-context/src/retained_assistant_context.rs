//! Typed framing for retained assistant evidence in independent Guardian snapshots.
//! Evidence stays in separate content parts to preserve its budgeting and source metadata.

use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Owns only the section delimiters; separately budgeted assistant entries supply the body.
pub(crate) struct RetainedAssistantContext;

impl ContextualUserFragment for RetainedAssistantContext {
    fn role(&self) -> &'static str {
        "user"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.retained_assistant_context".to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            ">>> RETAINED ASSISTANT CONTEXT START\n\n",
            ">>> RETAINED ASSISTANT CONTEXT END\n\n",
        )
    }

    fn body(&self) -> String {
        String::new()
    }
}
