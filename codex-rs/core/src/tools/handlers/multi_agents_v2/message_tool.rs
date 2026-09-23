//! Shared argument parsing and dispatch for the v2 agent messaging tools.
//!
//! `send_message` and `followup_task` share the same submission path and differ only in whether the
//! resulting `InterAgentCommunication` should wake the target immediately.

use super::analytics::ToolCallAnalytics;
use super::*;
use crate::TurnStartOptions;
use crate::agent::api::AgentInput;
use crate::agent::api::AgentTarget;
use crate::agent::api::SendRequest;
use crate::agent::child_config::build_agent_resume_config;
use crate::agent::types::MessageDeliveryMode;
use crate::tools::context::FunctionToolOutput;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Input for the MultiAgentV2 `send_message` tool.
pub(crate) struct SendMessageArgs {
    pub(crate) target: String,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Input for the MultiAgentV2 `followup_task` tool.
pub(crate) struct FollowupTaskArgs {
    pub(crate) target: String,
    pub(crate) message: String,
}

pub(super) fn message_content(message: String) -> Result<String, FunctionCallError> {
    if message.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "Empty message can't be sent to an agent".to_string(),
        ));
    }
    Ok(message)
}

/// Handles the shared MultiAgentV2 message flow for both `send_message` and `followup_task`.
pub(super) async fn handle_message_string_tool(
    invocation: ToolInvocation,
    mode: MessageDeliveryMode,
    target: String,
    message: String,
    analytics: &mut ToolCallAnalytics,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let message = message_content(message)?;
    let ToolInvocation {
        session,
        turn,
        call_id,
        source,
        ..
    } = invocation;
    let receiver_thread_id = resolve_agent_target(&session, &turn, &target).await?;
    analytics.set_receiver(receiver_thread_id);
    let resume_config =
        build_agent_resume_config(&turn).map_err(FunctionCallError::RespondToModel)?;
    let receipt = session
        .services
        .agent_control
        .send(SendRequest {
            caller: session.thread_id,
            target: AgentTarget::Id(receiver_thread_id),
            resume_config,
            input: AgentInput::Message {
                message: agent_message_from_tool(message, &source),
                mode,
            },
            start_options: TurnStartOptions {
                parent_turn_id: (mode == MessageDeliveryMode::TriggerTurn)
                    .then(|| turn.sub_id.clone()),
                root_turn_id: turn.turn_metadata_state.root_turn_id(),
                turn_trigger: turn.turn_metadata_state.current_turn_trigger(),
                cyber_access_program: turn.cyber_access_program,
                ..Default::default()
            },
        })
        .await
        .map_err(|err| collab_v2_agent_error(receiver_thread_id, err))?;
    let receiver_agent_path = receipt.metadata.agent_path.ok_or_else(|| {
        FunctionCallError::RespondToModel("target agent is missing an agent_path".to_string())
    })?;
    emit_sub_agent_activity(
        &session,
        &turn,
        SubAgentActivityItem {
            id: call_id,
            agent_thread_id: receiver_thread_id,
            agent_path: receiver_agent_path,
            kind: SubAgentActivityKind::Interacted,
        },
    )
    .await;

    Ok(FunctionToolOutput::from_text(String::new(), Some(true)))
}
