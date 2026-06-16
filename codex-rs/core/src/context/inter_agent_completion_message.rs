use codex_protocol::AgentPath;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterAgentCompletionMessage {
    task_name: AgentPath,
    sender: AgentPath,
    payload: String,
}

impl InterAgentCompletionMessage {
    pub(crate) fn new(task_name: AgentPath, sender: AgentPath, payload: impl Into<String>) -> Self {
        Self {
            task_name,
            sender,
            payload: payload.into(),
        }
    }
}

impl ContextualUserFragment for InterAgentCompletionMessage {
    fn role(&self) -> &'static str {
        "assistant"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        format!(
            "Message Type: FINAL_ANSWER\nTask name: {}\nSender: {}\nPayload:\n{}",
            self.task_name, self.sender, self.payload,
        )
    }
}
