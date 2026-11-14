use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RolloutBudgetContext {
    pub(crate) remaining_units: f64,
}

impl ContextualUserFragment for RolloutBudgetContext {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("rollout_budget.remaining_tokens".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<rollout_budget>\n", "\n</rollout_budget>")
    }

    fn body(&self) -> ContentItem {
        // "Weighted tokens" is the legacy wording. Provider-reported units can be opaque.
        input_text(format!(
            "You have {} weighted tokens left in the shared session token budget.",
            self.remaining_units
        ))
    }
}
