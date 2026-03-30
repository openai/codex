use super::*;
use crate::agent::WatchdogParentCompactionResult;

pub(crate) struct Handler;

#[async_trait]
impl ToolHandler for Handler {
    type Output = CompactParentContextResult;

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
        let _args: CompactParentContextArgs = parse_arguments(&arguments)?;
        let helper_thread_id = session.conversation_id;
        let result = session
            .services
            .agent_control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("compact_parent_context failed: {err}"))
            })?;
        Ok(CompactParentContextResult::from(result))
    }
}

#[derive(Debug, Deserialize)]
struct CompactParentContextArgs {
    #[serde(rename = "reason")]
    _reason: Option<String>,
    #[serde(rename = "evidence")]
    _evidence: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CompactParentContextResult {
    kind: &'static str,
    parent_thread_id: Option<String>,
    submission_id: Option<String>,
}

impl From<WatchdogParentCompactionResult> for CompactParentContextResult {
    fn from(value: WatchdogParentCompactionResult) -> Self {
        match value {
            WatchdogParentCompactionResult::NotWatchdogHelper => Self {
                kind: "not_watchdog_helper",
                parent_thread_id: None,
                submission_id: None,
            },
            WatchdogParentCompactionResult::ParentBusy { parent_thread_id } => Self {
                kind: "parent_busy",
                parent_thread_id: Some(parent_thread_id.to_string()),
                submission_id: None,
            },
            WatchdogParentCompactionResult::AlreadyInProgress { parent_thread_id } => Self {
                kind: "already_in_progress",
                parent_thread_id: Some(parent_thread_id.to_string()),
                submission_id: None,
            },
            WatchdogParentCompactionResult::Submitted {
                parent_thread_id,
                submission_id,
            } => Self {
                kind: "submitted",
                parent_thread_id: Some(parent_thread_id.to_string()),
                submission_id: Some(submission_id),
            },
        }
    }
}

impl ToolOutput for CompactParentContextResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "compact_parent_context")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "compact_parent_context")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "compact_parent_context")
    }
}
