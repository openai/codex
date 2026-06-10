use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetContext {
    window_id: u64,
    window_size: i64,
}

impl TokenBudgetContext {
    pub(crate) fn new(window_id: u64, window_size: i64) -> Self {
        Self {
            window_id,
            window_size,
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
        let window_size = self.window_size;
        format!("Current context window {window_id}, window size {window_size} tokens")
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
        format!("{tokens_left} tokens left in the current window")
    }
}
