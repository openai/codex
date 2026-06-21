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
        ("", "")
    }

    fn body(&self) -> String {
        let thread_id = self.thread_id;
        let first_window_id = self.first_window_id;
        let window_id = self.window_id;
        let mut lines = vec![
            format!("Thread id {thread_id}."),
            format!("First context window id: {first_window_id}"),
            format!("Current context window id: {window_id}"),
        ];
        if let Some(previous_window_id) = self.previous_window_id {
            lines.push(format!("Previous context window id: {previous_window_id}"));
        }
        if let Some(mcp_result) = &self.mcp_result {
            lines.push(mcp_result.clone());
        }
        lines.join("\n")
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
        ("", "")
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenBudgetReminder {
    message: String,
}

impl TokenBudgetReminder {
    pub(crate) fn new(message_template: &str, n_remaining: i64) -> Self {
        Self {
            message: message_template.replace("{n_remaining}", &n_remaining.to_string()),
        }
    }
}

impl ContextualUserFragment for TokenBudgetReminder {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.message.clone()
    }
}
