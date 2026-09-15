use super::analytics::ToolCallAnalytics;
use super::*;
use crate::agent::control::AgentInterruptError;
use crate::agent::control::AgentInterruptOutcome;
use crate::tools::handlers::multi_agents_spec::create_interrupt_agent_tool_v2;
use codex_tools::ToolSpec;

pub(crate) struct Handler;

impl ToolExecutor<ToolInvocation> for Handler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("interrupt_agent")
    }

    fn spec(&self) -> ToolSpec {
        create_interrupt_agent_tool_v2()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
            let mut analytics =
                ToolCallAnalytics::new(&invocation, CollabAgentTool::InterruptAgent);
            let result = handle_interrupt_agent(invocation, &mut analytics).await;
            analytics.finish(&result);
            result.map(boxed_tool_output)
        })
    }
}

async fn handle_interrupt_agent(
    invocation: ToolInvocation,
    analytics: &mut ToolCallAnalytics,
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
    analytics.set_receiver(agent_id);
    let AgentInterruptOutcome {
        agent_path,
        previous_status,
    } = session
        .services
        .agent_control
        .interrupt_spawned_agent(session.thread_id, agent_id)
        .await
        .map_err(|err| match err {
            AgentInterruptError::InvalidRequest(message) => {
                FunctionCallError::RespondToModel(message)
            }
            AgentInterruptError::Agent(err) => collab_agent_error(agent_id, err),
        })?;
    emit_sub_agent_activity(
        &session,
        &turn,
        SubAgentActivityItem {
            id: call_id,
            agent_thread_id: agent_id,
            agent_path,
            kind: SubAgentActivityKind::Interrupted,
        },
    )
    .await;

    Ok(InterruptAgentResult { previous_status })
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
    fn log_output(&self) -> String {
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
