use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentCommunicationKind;
use codex_protocol::protocol::AgentCommunicationMetadata;
use codex_protocol::protocol::AgentCommunicationState;
use codex_protocol::protocol::InterAgentCommunication;
use uuid::Uuid;

pub(crate) fn new_agent_communication_metadata(
    kind: AgentCommunicationKind,
    sender_thread_id: ThreadId,
    source_call_id: Option<&str>,
) -> AgentCommunicationMetadata {
    AgentCommunicationMetadata {
        id: Uuid::new_v4().to_string(),
        kind,
        sender_thread_id,
        source_call_id: source_call_id.map(str::to_owned),
    }
}

pub(crate) fn emit_agent_communication_created(
    communication: &InterAgentCommunication,
    receiver_thread_id: ThreadId,
) {
    let Some(metadata) = communication.agent_communication_metadata.as_ref() else {
        return;
    };
    emit_agent_communication_event(
        metadata,
        AgentCommunicationState::Created,
        receiver_thread_id,
        communication_content(communication),
    );
}

fn emit_agent_communication_event(
    metadata: &AgentCommunicationMetadata,
    state: AgentCommunicationState,
    receiver_thread_id: ThreadId,
    content: &str,
) {
    tracing::trace!(
        {
            event.name = "codex.agent_communication",
            communication_id = %metadata.id,
            kind = metadata.kind.as_str(),
            state = state.as_str(),
            sender_thread_id = %metadata.sender_thread_id,
            receiver_thread_id = %receiver_thread_id,
            content,
            source_call_id = metadata.source_call_id.as_deref(),
        },
        "agent communication updated"
    );
}

pub(crate) fn emit_agent_communication_enqueued(metadata: AgentCommunicationMetadata) {
    tracing::trace!(
        {
            event.name = "codex.agent_communication",
            communication_id = %metadata.id,
            state = AgentCommunicationState::Enqueued.as_str(),
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
