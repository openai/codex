use super::ContextualUserFragment;
use codex_protocol::ThreadId;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetContext {
    thread_id: ThreadId,
    first_window_id: Uuid,
    previous_window_id: Option<Uuid>,
    window_id: Uuid,
    mcp_result: Option<String>,
}

impl TokenBudgetContext {
    pub(crate) fn new(
        thread_id: ThreadId,
        first_window_id: Uuid,
        previous_window_id: Option<Uuid>,
        window_id: Uuid,
        mcp_result: Option<String>,
    ) -> Self {
        Self {
            thread_id,
            first_window_id,
            previous_window_id,
            window_id,
            mcp_result,
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
        let thread_id = self.thread_id;
        let first_window_id = self.first_window_id;
        let previous_window_id = self
            .previous_window_id
            .map_or_else(|| "none".to_string(), |window_id| window_id.to_string());
        let window_id = self.window_id;
        let mcp_result = self
            .mcp_result
            .as_deref()
            .map(|result| format!("\n{result}"))
            .unwrap_or_default();
        format!(
            "Thread id {thread_id}.\nFirst context window id {first_window_id}.\nPrevious context window id {previous_window_id}.\nCurrent context window id {window_id}.{mcp_result}"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetRemainingContext {
    tokens_left: Option<i64>,
}

impl TokenBudgetRemainingContext {
    pub(crate) fn new(tokens_left: i64) -> Self {
        Self {
            tokens_left: Some(tokens_left),
        }
    }

    pub(crate) fn unknown() -> Self {
        Self { tokens_left: None }
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
        match self.tokens_left {
            Some(tokens_left) => {
                format!("You have {tokens_left} tokens left in this context window.")
            }
            None => "You have unknown tokens left in this context window.".to_string(),
        }
    }
}
