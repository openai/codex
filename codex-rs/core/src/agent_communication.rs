use codex_protocol::ThreadId;
use codex_protocol::protocol::InterAgentCommunication;
use std::sync::Arc;

use crate::current_time::TimeProvider;

const AGENT_COMMUNICATION_TARGET: &str = "codex_core::agent_communication";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentCommunicationKind {
    Spawn,
    Message,
    Followup,
    Result,
}

impl AgentCommunicationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Message => "message",
            Self::Followup => "followup",
            Self::Result => "result",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentCommunicationContext {
    kind: AgentCommunicationKind,
    pub(crate) sender_thread_id: ThreadId,
}

impl AgentCommunicationContext {
    pub(crate) fn new(kind: AgentCommunicationKind, sender_thread_id: ThreadId) -> Self {
        Self {
            kind,
            sender_thread_id,
        }
    }
}

pub(crate) fn logging_enabled() -> bool {
    tracing::enabled!(target: AGENT_COMMUNICATION_TARGET, tracing::Level::TRACE)
}

pub(crate) fn emit_agent_communication_send(
    communication_id: &str,
    context: &AgentCommunicationContext,
    communication: &InterAgentCommunication,
    receiver_thread_id: ThreadId,
    simclock_time: Option<i64>,
) {
    tracing::trace!(
        target: AGENT_COMMUNICATION_TARGET,
        {
            event.name = "codex.agent_communication",
            communication_id,
            kind = context.kind.as_str(),
            state = "send",
            sender_thread_id = %context.sender_thread_id,
            receiver_thread_id = %receiver_thread_id,
            content = if communication.content.is_empty() {
                communication.encrypted_content.as_deref().unwrap_or_default()
            } else {
                communication.content.as_str()
            },
            simclock_time,
        },
        "agent communication"
    );
}

pub(crate) async fn read_simclock_time(
    time_provider: Arc<dyn TimeProvider>,
    thread_id: ThreadId,
) -> Option<i64> {
    time_provider
        .current_time(thread_id)
        .await
        .ok()
        .map(|time| time.timestamp())
}

pub(crate) fn emit_agent_communication_receive(communication_id: &str, simclock_time: Option<i64>) {
    tracing::trace!(
        target: AGENT_COMMUNICATION_TARGET,
        {
            event.name = "codex.agent_communication",
            communication_id,
            state = "receive",
            simclock_time,
        },
        "agent communication"
    );
}
