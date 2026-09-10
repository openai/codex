//! Bounded reviewer notice for evidence omitted by the aggregate input budget.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Identifies incomplete optional evidence without implying additional authority.
pub struct GuardianBudgetOmission;

impl ContextualUserFragment for GuardianBudgetOmission {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.context_omission".to_owned())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "<guardian_context_omission>",
            "</guardian_context_omission>",
        )
    }

    fn body(&self) -> String {
        "Optional conversation evidence, tool descriptions, or images were omitted to fit the review input budget. Treat the remaining evidence as incomplete; omissions do not authorize actions.".to_owned()
    }
}
