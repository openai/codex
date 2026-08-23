use codex_protocol::AgentPath;

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterAgentMessageType {
    Message,
    NewTask,
}

impl InterAgentMessageType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Message => "MESSAGE",
            Self::NewTask => "NEW_TASK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterAgentMessage {
    message_type: InterAgentMessageType,
    task_name: AgentPath,
    sender: AgentPath,
    payload: String,
}

impl InterAgentMessage {
    pub(crate) fn new(
        message_type: InterAgentMessageType,
        task_name: AgentPath,
        sender: AgentPath,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            message_type,
            task_name,
            sender,
            payload: payload.into(),
        }
    }
}

impl ContextualUserFragment for InterAgentMessage {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("multi_agent.inter_agent_message".to_string())
    }

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
            "Message Type: {}\nTask name: {}\nSender: {}\nPayload:\n{}",
            self.message_type.as_str(),
            self.task_name,
            self.sender,
            self.payload,
        )
    }
}
