use super::*;

pub(crate) struct Handler;

#[async_trait]
impl ToolHandler for Handler {
    type Output = WatchdogSelfCloseResult;

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
        let _args: WatchdogSelfCloseArgs = parse_arguments(&arguments)?;

        let helper_thread_id = session.conversation_id;
        if session
            .services
            .agent_control
            .watchdog_owner_for_active_helper(helper_thread_id)
            .await
            .is_none()
        {
            return Err(FunctionCallError::RespondToModel(
                "watchdog_self_close is only available in watchdog check-in threads.".to_string(),
            ));
        };

        let receiver_agent = session
            .services
            .agent_control
            .get_agent_metadata(helper_thread_id)
            .unwrap_or_default();

        session
            .send_event(
                &turn,
                CollabCloseBeginEvent {
                    call_id: call_id.clone(),
                    sender_thread_id: helper_thread_id,
                    receiver_thread_id: helper_thread_id,
                }
                .into(),
            )
            .await;

        let status = match session
            .services
            .agent_control
            .subscribe_status(helper_thread_id)
            .await
        {
            Ok(mut status_rx) => status_rx.borrow_and_update().clone(),
            Err(err) => {
                let status = session
                    .services
                    .agent_control
                    .get_status(helper_thread_id)
                    .await;
                session
                    .send_event(
                        &turn,
                        CollabCloseEndEvent {
                            call_id: call_id.clone(),
                            sender_thread_id: helper_thread_id,
                            receiver_thread_id: helper_thread_id,
                            receiver_agent_nickname: receiver_agent.agent_nickname.clone(),
                            receiver_agent_role: receiver_agent.agent_role.clone(),
                            status,
                        }
                        .into(),
                    )
                    .await;
                return Err(collab_agent_error(helper_thread_id, err));
            }
        };

        let result = session
            .services
            .agent_control
            .close_agent(helper_thread_id)
            .await
            .map_err(|err| collab_agent_error(helper_thread_id, err))
            .map(|_| ());

        let receiver_agent = session
            .services
            .agent_control
            .get_agent_metadata(helper_thread_id)
            .unwrap_or_default();
        session
            .send_event(
                &turn,
                CollabCloseEndEvent {
                    call_id,
                    sender_thread_id: helper_thread_id,
                    receiver_thread_id: helper_thread_id,
                    receiver_agent_nickname: receiver_agent.agent_nickname,
                    receiver_agent_role: receiver_agent.agent_role,
                    status: status.clone(),
                }
                .into(),
            )
            .await;

        result?;

        Ok(WatchdogSelfCloseResult {
            previous_status: status,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WatchdogSelfCloseArgs {}

#[derive(Debug, Serialize)]
pub(crate) struct WatchdogSelfCloseResult {
    previous_status: AgentStatus,
}

impl ToolOutput for WatchdogSelfCloseResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "watchdog_self_close")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "watchdog_self_close")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "watchdog_self_close")
    }
}
