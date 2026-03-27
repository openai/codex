//! Shared argument parsing and dispatch for the v2 text-only agent messaging tools.
//!
//! `send_message` and `assign_task` intentionally expose the same input shape and differ only in
//! whether the resulting `InterAgentCommunication` should wake the target immediately.

use super::*;
use crate::agent::control::render_input_preview;
use codex_protocol::protocol::InterAgentCommunication;

#[derive(Clone, Copy)]
pub(crate) enum MessageDeliveryMode {
    QueueOnly,
    TriggerTurn,
}

impl MessageDeliveryMode {
    /// Returns the model-visible error message for non-text inputs.
    fn unsupported_items_error(self) -> &'static str {
        match self {
            Self::QueueOnly => "send_message only supports text content in MultiAgentV2 for now",
            Self::TriggerTurn => "assign_task only supports text content in MultiAgentV2 for now",
        }
    }

    /// Returns whether the produced communication should start a turn immediately.
    fn apply(self, communication: InterAgentCommunication) -> InterAgentCommunication {
        match self {
            Self::QueueOnly => InterAgentCommunication {
                trigger_turn: false,
                ..communication
            },
            Self::TriggerTurn => InterAgentCommunication {
                trigger_turn: true,
                ..communication
            },
        }
    }
}

#[derive(Debug, Deserialize)]
/// Input shared by the MultiAgentV2 `send_message` and `assign_task` tools.
pub(crate) struct MessageToolArgs {
    pub(crate) target: String,
    pub(crate) items: Vec<UserInput>,
    #[serde(default)]
    pub(crate) interrupt: bool,
}

#[derive(Debug, Serialize)]
/// Tool result shared by the MultiAgentV2 message-delivery tools.
pub(crate) struct MessageToolResult {
    submission_id: String,
}

impl ToolOutput for MessageToolResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "multi_agent_message")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "multi_agent_message")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "multi_agent_message")
    }
}

/// Validates that the tool input is non-empty text-only content and returns its preview string.
fn text_content(
    items: &[UserInput],
    mode: MessageDeliveryMode,
) -> Result<String, FunctionCallError> {
    if items.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "Items can't be empty".to_string(),
        ));
    }
    if items
        .iter()
        .all(|item| matches!(item, UserInput::Text { .. }))
    {
        return Ok(render_input_preview(&(items.to_vec().into())));
    }
    Err(FunctionCallError::RespondToModel(
        mode.unsupported_items_error().to_string(),
    ))
}

/// Handles the shared MultiAgentV2 text-message flow for both `send_message` and `assign_task`.
pub(crate) async fn handle_message_tool(
    invocation: ToolInvocation,
    mode: MessageDeliveryMode,
) -> Result<MessageToolResult, FunctionCallError> {
    let ToolInvocation {
        session,
        turn,
        payload,
        call_id,
        ..
    } = invocation;
    let arguments = function_arguments(payload)?;
    let args: MessageToolArgs = parse_arguments(&arguments)?;
    let receiver_thread_id = resolve_agent_target(&session, &turn, &args.target).await?;
    let prompt = text_content(&args.items, mode)?;
    let receiver_agent = session
        .services
        .agent_control
        .get_agent_metadata(receiver_thread_id)
        .unwrap_or_default();
    if args.interrupt {
        session
            .services
            .agent_control
            .interrupt_agent(receiver_thread_id)
            .await
            .map_err(|err| collab_agent_error(receiver_thread_id, err))?;
    }
    session
        .send_event(
            &turn,
            CollabAgentInteractionBeginEvent {
                call_id: call_id.clone(),
                sender_thread_id: session.conversation_id,
                receiver_thread_id,
                prompt: prompt.clone(),
            }
            .into(),
        )
        .await;
    let receiver_agent_path = receiver_agent.agent_path.clone().ok_or_else(|| {
        FunctionCallError::RespondToModel("target agent is missing an agent_path".to_string())
    })?;
    let communication = InterAgentCommunication::new(
        turn.session_source
            .get_agent_path()
            .unwrap_or_else(AgentPath::root),
        receiver_agent_path,
        Vec::new(),
        prompt.clone(),
        /*trigger_turn*/ true,
    );
    let result = session
        .services
        .agent_control
        .send_inter_agent_communication(receiver_thread_id, mode.apply(communication))
        .await
        .map_err(|err| collab_agent_error(receiver_thread_id, err));
    let status = session
        .services
        .agent_control
        .get_status(receiver_thread_id)
        .await;
    session
        .send_event(
            &turn,
            CollabAgentInteractionEndEvent {
                call_id,
                sender_thread_id: session.conversation_id,
                receiver_thread_id,
                receiver_agent_nickname: receiver_agent.agent_nickname,
                receiver_agent_role: receiver_agent.agent_role,
                prompt,
                status,
            }
            .into(),
        )
        .await;
    let submission_id = result?;

    Ok(MessageToolResult { submission_id })
}
