use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentCommunicationKind;
use codex_protocol::protocol::AgentCommunicationRecord;
use codex_protocol::protocol::AgentCommunicationState;
use codex_protocol::protocol::InterAgentCommunication;
use uuid::Uuid;

use crate::session::Session;

/// Receives live agent-to-agent communication lifecycle records.
///
/// Implementations must return quickly and must not block the agent runtime. Records sent to this
/// sink are intentionally not persisted in rollouts.
pub trait AgentCommunicationSink: Send + Sync {
    /// Publishes one live communication lifecycle record without blocking.
    fn emit(&self, record: AgentCommunicationRecord);
}

/// Drops agent communication lifecycle records when the host does not expose them.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAgentCommunicationSink;

impl AgentCommunicationSink for NoopAgentCommunicationSink {
    fn emit(&self, _record: AgentCommunicationRecord) {}
}

impl Session {
    pub(crate) fn new_agent_communication(
        &self,
        kind: AgentCommunicationKind,
        sender_thread_id: ThreadId,
        receiver_thread_id: ThreadId,
        communication: &InterAgentCommunication,
        source_call_id: Option<String>,
    ) -> AgentCommunicationRecord {
        let content = if communication.content.is_empty() {
            communication.encrypted_content.clone().unwrap_or_default()
        } else {
            communication.content.clone()
        };
        AgentCommunicationRecord {
            id: Uuid::new_v4().to_string(),
            kind,
            state: AgentCommunicationState::Created,
            sender_thread_id,
            receiver_thread_id,
            content,
            source_call_id,
            occurred_at_ms: crate::turn_timing::now_unix_timestamp_ms(),
        }
    }

    pub(crate) fn emit_agent_communication(&self, record: AgentCommunicationRecord) {
        self.services.agent_communication_sink.emit(record);
    }

    pub(crate) fn emit_agent_communication_enqueued(&self, mut record: AgentCommunicationRecord) {
        record.state = AgentCommunicationState::Enqueued;
        record.occurred_at_ms = crate::turn_timing::now_unix_timestamp_ms();
        self.emit_agent_communication(record);
    }
}
