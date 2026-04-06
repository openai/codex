use super::*;

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = WatchdogSnoozeResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: WatchdogSnoozeArgs = parse_arguments(&arguments)?;
        let helper_thread_id = session.conversation_id;
        let Some(result) = session
            .services
            .agent_control
            .snooze_watchdog_helper(helper_thread_id, args.delay_seconds)
            .await
        else {
            return Err(FunctionCallError::RespondToModel(
                "watchdog.snooze is only available in watchdog check-in threads.".to_string(),
            ));
        };
        session.mark_turn_used_watchdog_terminal_tool();
        let _ = args.reason;
        Ok(WatchdogSnoozeResult {
            target_thread_id: result.target_thread_id.to_string(),
            delay_seconds: result.delay_seconds,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WatchdogSnoozeArgs {
    delay_seconds: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WatchdogSnoozeResult {
    target_thread_id: String,
    delay_seconds: u64,
}

impl ToolOutput for WatchdogSnoozeResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "snooze")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "snooze")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "snooze")
    }
}
