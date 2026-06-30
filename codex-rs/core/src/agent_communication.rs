use codex_protocol::ThreadId;
use codex_protocol::protocol::InterAgentCommunication;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentCommunicationKind {
    InitialTask,
    Message,
    Followup,
    Result,
}

impl AgentCommunicationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::InitialTask => "initialTask",
            Self::Message => "message",
            Self::Followup => "followup",
            Self::Result => "result",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentCommunicationContext {
    id: String,
    kind: AgentCommunicationKind,
    sender_thread_id: ThreadId,
    source_call_id: Option<String>,
}

impl AgentCommunicationContext {
    pub(crate) fn from_tool_call(
        kind: AgentCommunicationKind,
        sender_thread_id: ThreadId,
        source_call_id: &str,
    ) -> Self {
        Self::new(kind, sender_thread_id, Some(source_call_id.to_string()))
    }

    pub(crate) fn without_source_call(
        kind: AgentCommunicationKind,
        sender_thread_id: ThreadId,
    ) -> Self {
        Self::new(kind, sender_thread_id, None)
    }

    fn new(
        kind: AgentCommunicationKind,
        sender_thread_id: ThreadId,
        source_call_id: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            kind,
            sender_thread_id,
            source_call_id,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

pub(crate) fn emit_agent_communication_created(
    context: &AgentCommunicationContext,
    communication: &InterAgentCommunication,
    receiver_thread_id: ThreadId,
) {
    tracing::trace!(
        {
            event.name = "codex.agent_communication",
            communication_id = %context.id,
            kind = context.kind.as_str(),
            state = "created",
            sender_thread_id = %context.sender_thread_id,
            receiver_thread_id = %receiver_thread_id,
            content = communication_content(communication),
            source_call_id = context.source_call_id.as_deref(),
        },
        "agent communication updated"
    );
}

pub(crate) fn emit_agent_communication_enqueued(id: &str) {
    tracing::trace!(
        {
            event.name = "codex.agent_communication",
            communication_id = id,
            state = "enqueued",
        },
        "agent communication updated"
    );
}

pub(crate) fn communication_content(communication: &InterAgentCommunication) -> &str {
    if communication.content.is_empty() {
        communication
            .encrypted_content
            .as_deref()
            .unwrap_or_default()
    } else {
        &communication.content
    }
}

#[cfg(test)]
#[path = "agent_communication_tests.rs"]
mod tests;
