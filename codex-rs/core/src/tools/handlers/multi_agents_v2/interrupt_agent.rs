use super::*;
use crate::tools::handlers::multi_agents_spec::create_interrupt_agent_tool_v2;
use crate::turn_timing::now_unix_timestamp_ms;
use codex_protocol::error::CodexErr;
use codex_tools::ToolSpec;

pub(crate) struct Handler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for Handler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("interrupt_agent")
    }

    fn spec(&self) -> ToolSpec {
        create_interrupt_agent_tool_v2()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        handle_interrupt_agent(invocation)
            .await
            .map(boxed_tool_output)
    }
}

async fn handle_interrupt_agent(
    invocation: ToolInvocation,
) -> Result<InterruptAgentResult, FunctionCallError> {
    let ToolInvocation {
        session,
        turn,
        payload,
        call_id,
        ..
    } = invocation;
    let arguments = function_arguments(payload)?;
    let args: InterruptAgentArgs = parse_arguments(&arguments)?;
    let agent_id = resolve_agent_target(&session, &turn, &args.target).await?;
    let receiver_agent = session
        .services
        .agent_control
        .ensure_agent_known(agent_id)
        .map_err(|err| collab_agent_error(agent_id, err))?;
    if receiver_agent
        .agent_path
        .as_ref()
        .is_some_and(AgentPath::is_root)
    {
        return Err(FunctionCallError::RespondToModel(
            "root is not a spawned agent".to_string(),
        ));
    }
    if agent_id == session.thread_id {
        return Err(FunctionCallError::RespondToModel(
            "an agent cannot interrupt itself; return your result and let the parent interrupt you if needed"
                .to_string(),
        ));
    }
    session
        .send_event(
            &turn,
            CollabCloseBeginEvent {
                call_id: call_id.clone(),
                started_at_ms: now_unix_timestamp_ms(),
                sender_thread_id: session.thread_id,
                receiver_thread_id: agent_id,
            }
            .into(),
        )
        .await;
    let status = session.services.agent_control.get_status(agent_id).await;
    let result = match session
        .services
        .agent_control
        .interrupt_agent(agent_id)
        .await
    {
        Ok(_) | Err(CodexErr::ThreadNotFound(_)) | Err(CodexErr::InternalAgentDied) => Ok(()),
        Err(err) => Err(collab_agent_error(agent_id, err)),
    };
    session
        .send_event(
            &turn,
            CollabCloseEndEvent {
                call_id,
                completed_at_ms: now_unix_timestamp_ms(),
                sender_thread_id: session.thread_id,
                receiver_thread_id: agent_id,
                receiver_agent_nickname: receiver_agent.agent_nickname,
                receiver_agent_role: receiver_agent.agent_role,
                status: status.clone(),
            }
            .into(),
        )
        .await;
    result?;

    Ok(InterruptAgentResult {
        previous_status: status,
    })
}

impl CoreToolRuntime for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InterruptAgentArgs {
    target: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InterruptAgentResult {
    pub(crate) previous_status: AgentStatus,
}

impl ToolOutput for InterruptAgentResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "interrupt_agent")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "interrupt_agent")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "interrupt_agent")
    }
}
