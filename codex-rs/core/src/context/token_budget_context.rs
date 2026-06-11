use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetContext {
    window_id: u64,
    tokens_left: i64,
}

impl TokenBudgetContext {
    pub(crate) fn new(window_id: u64, tokens_left: i64) -> Self {
        Self {
            window_id,
            tokens_left,
        }
    }
}

impl ContextualUserFragment for TokenBudgetContext {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<token_budget>\n", "\n</token_budget>")
    }

    fn body(&self) -> String {
        let window_id = self.window_id;
        let tokens_left = self.tokens_left;
        format!(
            "Current context window {window_id}.\nYou have {tokens_left} tokens left in this context window."
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetRemainingContext {
    tokens_left: i64,
}

impl TokenBudgetRemainingContext {
    pub(crate) fn new(tokens_left: i64) -> Self {
        Self { tokens_left }
    }
}

impl ContextualUserFragment for TokenBudgetRemainingContext {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<token_budget>\n", "\n</token_budget>")
    }

    fn body(&self) -> String {
        let tokens_left = self.tokens_left;
        format!("You have {tokens_left} tokens left in this context window.")
    }
}
