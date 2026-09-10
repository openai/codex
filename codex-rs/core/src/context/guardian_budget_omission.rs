//! Bounded reviewer notice for evidence omitted by the aggregate input budget.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Identifies incomplete evidence without implying additional authority.
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
        "Conversation evidence, tool descriptions, or images were omitted or shortened to fit the review input budget. User instructions and prior approvals may be incomplete where marked. Do not infer authorization from missing evidence or treat a partial grant as overriding an omitted restriction.".to_owned()
    }
}
