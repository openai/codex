use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RolloutBudgetContext {
    pub(crate) remaining_tokens: i64,
}

impl ContextualUserFragment for RolloutBudgetContext {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<rollout_budget>\n", "\n</rollout_budget>")
    }

    fn body(&self) -> String {
        format!(
            "You have {} weighted tokens left in the shared session token budget.",
            self.remaining_tokens
        )
    }
}
