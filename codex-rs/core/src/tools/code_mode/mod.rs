mod delegate;
mod execute_handler;
pub(crate) mod execute_spec;
mod response_adapter;
mod wait_handler;
pub(crate) mod wait_spec;

use std::sync::Arc;
use std::time::Duration;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::RuntimeResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::function_tool::FunctionCallError;
use crate::original_image_detail::can_request_original_image_detail;
use crate::original_image_detail::sanitize_original_image_detail as sanitize_image_detail_items;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use crate::unified_exec::resolve_max_tokens;
use codex_protocol::openai_models::ToolMode;
use codex_tools::ToolName;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::formatted_truncate_text_content_items_with_policy;
use codex_utils_output_truncation::truncate_function_output_items_with_policy;

use delegate::CodeModeDispatchBroker;
use delegate::CodeModeDispatchWorker;
pub(crate) use execute_handler::CodeModeExecuteHandler;
use response_adapter::into_function_call_output_content_items;
pub(crate) use wait_handler::CodeModeWaitHandler;

pub(crate) const PUBLIC_TOOL_NAME: &str = codex_code_mode::PUBLIC_TOOL_NAME;
pub(crate) const WAIT_TOOL_NAME: &str = codex_code_mode::WAIT_TOOL_NAME;
pub(crate) const DEFAULT_WAIT_YIELD_TIME_MS: u64 = codex_code_mode::DEFAULT_WAIT_YIELD_TIME_MS;

/// Returns true for the un-namespaced code-mode `exec` tool.
pub(crate) fn is_exec_tool_name(tool_name: &ToolName) -> bool {
    tool_name.namespace.is_none() && tool_name.name == PUBLIC_TOOL_NAME
}

#[derive(Clone)]
pub(crate) struct ExecContext {
    pub(super) session: Arc<Session>,
    pub(super) turn: Arc<TurnContext>,
}

pub(crate) struct CodeModeService {
    session: Option<Arc<dyn CodeModeSession>>,
    dispatch_broker: Arc<CodeModeDispatchBroker>,
}

impl CodeModeService {
    pub(crate) fn new(config: codex_code_mode::CodeModeSessionConfig) -> Self {
        let dispatch_broker = Arc::new(CodeModeDispatchBroker::new());
        Self {
            session: Some(Arc::new(
                codex_code_mode::CodeModeService::with_delegate_and_config(
                    dispatch_broker.clone(),
                    config,
                ),
            )),
            dispatch_broker,
        }
    }

    pub(crate) async fn execute(
        &self,
        request: codex_code_mode::ExecuteRequest,
    ) -> Result<codex_code_mode::StartedCell, String> {
        self.session()?.execute(request).await
    }

    pub(crate) async fn wait(
        &self,
        request: codex_code_mode::WaitRequest,
    ) -> Result<codex_code_mode::WaitOutcome, String> {
        self.session()?.wait(request).await
    }

    pub(crate) async fn terminate(
        &self,
        cell_id: CellId,
    ) -> Result<codex_code_mode::WaitOutcome, String> {
        self.session()?.terminate(cell_id).await
    }

    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        match &self.session {
            Some(session) => session.shutdown().await,
            None => Ok(()),
        }
    }

    pub(crate) fn mark_cell_ready_for_dispatch(&self, cell_id: &codex_code_mode::CellId) {
        self.dispatch_broker.mark_cell_ready_for_dispatch(cell_id);
    }

    pub(crate) fn finish_cell_dispatch(&self, cell_id: &CellId) {
        self.dispatch_broker.close_cell(cell_id);
    }

    pub(crate) fn start_turn_worker(
        &self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        router: Arc<ToolRouter>,
        tracker: SharedTurnDiffTracker,
    ) -> Option<CodeModeDispatchWorker> {
        if !matches!(turn.tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly)
            || self.session.is_none()
        {
            return None;
        }

        let exec = ExecContext {
            session: Arc::clone(session),
            turn: Arc::clone(turn),
        };
        Some(
            self.dispatch_broker
                .start_turn_worker(exec, router, tracker),
        )
    }

    fn session(&self) -> Result<&Arc<dyn CodeModeSession>, String> {
        self.session
            .as_ref()
            .ok_or_else(|| "code mode is unavailable".to_string())
    }
}

pub(super) async fn handle_runtime_response(
    exec: &ExecContext,
    response: RuntimeResponse,
    max_output_tokens: Option<usize>,
    started_at: std::time::Instant,
) -> Result<FunctionToolOutput, String> {
    let script_status = format_script_status(&response);

    match response {
        RuntimeResponse::Yielded { content_items, .. } => {
            let mut content_items = into_function_call_output_content_items(content_items);
            sanitize_runtime_image_detail(exec.turn.as_ref(), &mut content_items);
            content_items = truncate_code_mode_result(content_items, max_output_tokens);
            prepend_script_status(&mut content_items, &script_status, started_at.elapsed());
            Ok(FunctionToolOutput::from_content(content_items, Some(true)))
        }
        RuntimeResponse::Terminated { content_items, .. } => {
            let mut content_items = into_function_call_output_content_items(content_items);
            sanitize_runtime_image_detail(exec.turn.as_ref(), &mut content_items);
            content_items = truncate_code_mode_result(content_items, max_output_tokens);
            prepend_script_status(&mut content_items, &script_status, started_at.elapsed());
            Ok(FunctionToolOutput::from_content(content_items, Some(true)))
        }
        RuntimeResponse::Result {
            content_items,
            error_text,
            ..
        } => {
            let mut content_items = into_function_call_output_content_items(content_items);
            sanitize_runtime_image_detail(exec.turn.as_ref(), &mut content_items);
            let success = error_text.is_none();
            if let Some(error_text) = error_text {
                content_items.push(FunctionCallOutputContentItem::InputText {
                    text: format!("Script error:\n{error_text}"),
                });
            }
            content_items = truncate_code_mode_result(content_items, max_output_tokens);
            prepend_script_status(&mut content_items, &script_status, started_at.elapsed());
            Ok(FunctionToolOutput::from_content(
                content_items,
                Some(success),
            ))
        }
    }
}

fn sanitize_runtime_image_detail(turn: &TurnContext, items: &mut [FunctionCallOutputContentItem]) {
    sanitize_image_detail_items(can_request_original_image_detail(&turn.model_info), items);
}

fn format_script_status(response: &RuntimeResponse) -> String {
    match response {
        RuntimeResponse::Yielded { cell_id, .. } => {
            format!("Script running with cell ID {cell_id}")
        }
        RuntimeResponse::Terminated { .. } => "Script terminated".to_string(),
        RuntimeResponse::Result { error_text, .. } => {
            if error_text.is_none() {
                "Script completed".to_string()
            } else {
                "Script failed".to_string()
            }
        }
    }
}

fn prepend_script_status(
    content_items: &mut Vec<FunctionCallOutputContentItem>,
    status: &str,
    wall_time: Duration,
) {
    let wall_time_seconds = ((wall_time.as_secs_f32()) * 10.0).round() / 10.0;
    let header = format!("{status}\nWall time {wall_time_seconds:.1} seconds\nOutput:\n");
    content_items.insert(0, FunctionCallOutputContentItem::InputText { text: header });
}

fn truncate_code_mode_result(
    items: Vec<FunctionCallOutputContentItem>,
    max_output_tokens: Option<usize>,
) -> Vec<FunctionCallOutputContentItem> {
    let max_output_tokens = resolve_max_tokens(max_output_tokens);
    let policy = TruncationPolicy::Tokens(max_output_tokens);
    if items
        .iter()
        .all(|item| matches!(item, FunctionCallOutputContentItem::InputText { .. }))
    {
        let (truncated_items, _) =
            formatted_truncate_text_content_items_with_policy(&items, policy);
        return truncated_items;
    }

    truncate_function_output_items_with_policy(&items, policy)
}

async fn call_nested_tool(
    _exec: ExecContext,
    tool_runtime: ToolCallRuntime,
    invocation: CodeModeNestedToolCall,
    cancellation_token: CancellationToken,
) -> Result<JsonValue, FunctionCallError> {
    let CodeModeNestedToolCall {
        cell_id,
        runtime_tool_call_id,
        tool_name,
        tool_kind,
        input,
    } = invocation;
    if is_exec_tool_name(&tool_name) {
        return Err(FunctionCallError::RespondToModel(format!(
            "{PUBLIC_TOOL_NAME} cannot invoke itself"
        )));
    }

    let payload = match build_nested_tool_payload(tool_kind, &tool_name, input) {
        Ok(payload) => payload,
        Err(error) => return Err(FunctionCallError::RespondToModel(error)),
    };

    let call = ToolCall {
        tool_name,
        call_id: format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4()),
        payload,
    };
    let result = tool_runtime
        .handle_tool_call_with_source(
            call,
            ToolCallSource::CodeMode {
                cell_id: cell_id.to_string(),
                runtime_tool_call_id,
            },
            cancellation_token,
        )
        .await?;
    Ok(result.code_mode_result())
}

fn build_nested_tool_payload(
    tool_kind: CodeModeToolKind,
    tool_name: &ToolName,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    match tool_kind {
        CodeModeToolKind::Function => build_function_tool_payload(tool_name, input),
        CodeModeToolKind::Freeform => build_freeform_tool_payload(tool_name, input),
    }
}

fn build_function_tool_payload(
    tool_name: &ToolName,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let arguments = serialize_function_tool_arguments(tool_name, input)?;
    Ok(ToolPayload::Function { arguments })
}

fn serialize_function_tool_arguments(
    tool_name: &ToolName,
    input: Option<JsonValue>,
) -> Result<String, String> {
    match input {
        None => Ok("{}".to_string()),
        Some(JsonValue::Object(map)) => serde_json::to_string(&JsonValue::Object(map))
            .map_err(|err| format!("failed to serialize tool `{tool_name}` arguments: {err}")),
        Some(_) => Err(format!(
            "tool `{tool_name}` expects a JSON object for arguments"
        )),
    }
}

fn build_freeform_tool_payload(
    tool_name: &ToolName,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    match input {
        Some(JsonValue::String(input)) => Ok(ToolPayload::Custom { input }),
        _ => Err(format!("tool `{tool_name}` expects a string input")),
    }
}

#[cfg(test)]
mod tests {
    use super::build_nested_tool_payload;
    use crate::tools::context::ToolPayload;
    use codex_code_mode::CodeModeToolKind;
    use codex_tools::ToolName;
    use serde_json::json;

    #[test]
    fn build_nested_tool_payload_uses_function_kind() {
        let payload = build_nested_tool_payload(
            CodeModeToolKind::Function,
            &ToolName::plain("example"),
            Some(json!({ "value": 1 })),
        )
        .expect("function payload should serialize");

        match payload {
            ToolPayload::Function { arguments } => {
                assert_eq!(arguments, r#"{"value":1}"#.to_string());
            }
            other => panic!("expected function payload, got {other:?}"),
        }
    }

    #[test]
    fn build_nested_tool_payload_uses_freeform_kind() {
        let payload = build_nested_tool_payload(
            CodeModeToolKind::Freeform,
            &ToolName::plain("example"),
            Some(json!("hello")),
        )
        .expect("freeform payload should preserve string input");

        match payload {
            ToolPayload::Custom { input } => {
                assert_eq!(input, "hello".to_string());
            }
            other => panic!("expected freeform payload, got {other:?}"),
        }
    }
}
