use super::*;
use crate::agent::next_thread_spawn_depth;

pub(crate) struct Handler;

#[async_trait]
impl ToolHandler for Handler {
    type Output = ResumeAgentResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: ResumeAgentArgs = parse_arguments(&arguments)?;
        let receiver_thread_id = ThreadId::from_string(&args.id).map_err(|err| {
            FunctionCallError::RespondToModel(format!("invalid agent id {}: {err:?}", args.id))
        })?;
        let receiver_agent = session
            .services
            .agent_control
            .get_agent_metadata(receiver_thread_id)
            .unwrap_or_default();
        let child_depth = next_thread_spawn_depth(&turn.session_source);
        let max_depth = turn.config.agent_max_depth;
        if exceeds_thread_spawn_depth_limit(child_depth, max_depth) {
            return Err(FunctionCallError::RespondToModel(
                "Agent depth limit reached. Solve the task yourself.".to_string(),
            ));
        }

        session
            .send_event(
                &turn,
                CollabResumeBeginEvent {
                    call_id: call_id.clone(),
                    sender_thread_id: session.conversation_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent.agent_nickname.clone(),
                    receiver_agent_role: receiver_agent.agent_role.clone(),
                }
                .into(),
            )
            .await;

        let mut status = session
            .services
            .agent_control
            .get_status(receiver_thread_id)
            .await;
        let (receiver_agent, error) = if matches!(status, AgentStatus::NotFound) {
            match try_resume_closed_agent(&session, &turn, receiver_thread_id, child_depth).await {
                Ok(()) => {
                    status = session
                        .services
                        .agent_control
                        .get_status(receiver_thread_id)
                        .await;
                    (
                        session
                            .services
                            .agent_control
                            .get_agent_metadata(receiver_thread_id)
                            .unwrap_or(receiver_agent),
                        None,
                    )
                }
                Err(err) => {
                    status = session
                        .services
                        .agent_control
                        .get_status(receiver_thread_id)
                        .await;
                    (receiver_agent, Some(err))
                }
            }
        } else {
            (receiver_agent, None)
        };
        session
            .send_event(
                &turn,
                CollabResumeEndEvent {
                    call_id,
                    sender_thread_id: session.conversation_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent.agent_nickname,
                    receiver_agent_role: receiver_agent.agent_role,
                    status: status.clone(),
                }
                .into(),
            )
            .await;

        if let Some(err) = error {
            return Err(err);
        }
        turn.session_telemetry
            .counter("codex.multi_agent.resume", /*inc*/ 1, &[]);

        Ok(ResumeAgentResult { status })
    }
}

#[derive(Debug, Deserialize)]
struct ResumeAgentArgs {
    id: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ResumeAgentResult {
    pub(crate) status: AgentStatus,
}

impl ToolOutput for ResumeAgentResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "resume_agent")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "resume_agent")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "resume_agent")
    }
}

async fn try_resume_closed_agent(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    receiver_thread_id: ThreadId,
    child_depth: i32,
) -> Result<(), FunctionCallError> {
    let config = build_agent_resume_config(turn.as_ref(), child_depth)?;
    session
        .services
        .agent_control
        .resume_agent_from_rollout(
            config,
            receiver_thread_id,
            thread_spawn_source(
                session.conversation_id,
                &turn.session_source,
                child_depth,
                /*agent_role*/ None,
                /*task_name*/ None,
            )?,
        )
        .await
        .map(|_| ())
        .map_err(|err| collab_agent_error(receiver_thread_id, err))
}
