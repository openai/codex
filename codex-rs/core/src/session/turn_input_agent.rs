//! Admits reply-bearing agent follow-ups through native message delivery.
//! Acceptance means queued by Core; the recipient can consume the input in a later turn.
//! Follow-ups retain the recipient's settings and pass turn-start metadata to native delivery.

use crate::PendingAgentMessage;
use crate::WithTurnExtensionData;
use crate::agent::types::AgentMessage;
use crate::agent::types::MessageDeliveryMode;
use crate::session::handlers;
use crate::session::session::Session;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::turn_input::NotSubmittedReason;
use codex_protocol::turn_input::TurnInput;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use std::sync::Arc;

pub(super) async fn submit(
    session: &Arc<Session>,
    request: WithTurnExtensionData<TurnInputRequest>,
    submission_id: String,
) -> CodexResult<TurnInputSubmission> {
    let WithTurnExtensionData { request, .. } = request;
    let TurnInputRequest { input, start, .. } = request;
    let TurnInput::InterAgentCommunication(mut communication) = input else {
        unreachable!("agent submission requires an agent message");
    };
    // CodexThread already checks local agent capacity before submitting this operation.
    // Preserve the host's drain gate when accepting work for an idle recipient.
    let _admission = if session.active_turn.lock().await.is_none() {
        let Some(admission) = session.services.extensions.admit_turn_start() else {
            return Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining,
            });
        };
        Some(admission)
    } else {
        None
    };
    if communication.encrypted_content.is_none() {
        communication.content = AgentMessage::Plaintext(std::mem::take(&mut communication.content))
            .into_communication(
                communication.author.clone(),
                communication.recipient.clone(),
                MessageDeliveryMode::TriggerTurn,
            )
            .content;
    }
    // Calling start-or-steer requests work.
    communication.trigger_turn = true;
    let pending = PendingAgentMessage::from(communication);
    // Bound the future layout depth of this additional native-delivery path.
    Box::pin(handlers::inter_agent_communication(
        session,
        submission_id,
        pending,
        start,
    ))
    .await;
    Ok(TurnInputSubmission::Accepted)
}
