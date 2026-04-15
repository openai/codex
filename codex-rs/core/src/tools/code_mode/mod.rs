mod execute_handler;
mod response_adapter;
mod wait_handler;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_code_mode::CodeModeTurnHost;
use codex_code_mode::RuntimeResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::tools::ToolRouter;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use crate::tools::router::ToolRouterParams;
use crate::unified_exec::resolve_max_tokens;
use codex_features::Feature;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::collect_code_mode_tool_definitions;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::formatted_truncate_text_content_items_with_policy;
use codex_utils_output_truncation::truncate_function_output_items_with_policy;

pub(crate) use execute_handler::CodeModeExecuteHandler;
use response_adapter::into_function_call_output_content_items;
pub(crate) use wait_handler::CodeModeWaitHandler;

pub(crate) const PUBLIC_TOOL_NAME: &str = codex_code_mode::PUBLIC_TOOL_NAME;
pub(crate) const WAIT_TOOL_NAME: &str = codex_code_mode::WAIT_TOOL_NAME;
pub(crate) const DEFAULT_WAIT_YIELD_TIME_MS: u64 = codex_code_mode::DEFAULT_WAIT_YIELD_TIME_MS;

#[derive(Clone)]
pub(crate) struct ExecContext {
    pub(super) session: Arc<Session>,
    pub(super) turn: Arc<TurnContext>,
}

pub(crate) struct CodeModeService {
    inner: codex_code_mode::CodeModeService,
}

impl CodeModeService {
    pub(crate) fn new(_js_repl_node_path: Option<PathBuf>) -> Self {
        Self {
            inner: codex_code_mode::CodeModeService::new(),
        }
    }

    pub(crate) async fn stored_values(&self) -> std::collections::HashMap<String, JsonValue> {
        self.inner.stored_values().await
    }

    pub(crate) async fn replace_stored_values(
        &self,
        values: std::collections::HashMap<String, JsonValue>,
    ) {
        self.inner.replace_stored_values(values).await;
    }

    pub(crate) async fn execute(
        &self,
        request: codex_code_mode::ExecuteRequest,
    ) -> Result<RuntimeResponse, String> {
        self.inner.execute(request).await
    }

    pub(crate) async fn wait(
        &self,
        request: codex_code_mode::WaitRequest,
    ) -> Result<RuntimeResponse, String> {
        self.inner.wait(request).await
    }

    pub(crate) async fn start_turn_worker(
        &self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        router: Arc<ToolRouter>,
        tracker: SharedTurnDiffTracker,
    ) -> Option<codex_code_mode::CodeModeTurnWorker> {
        if !turn.features.enabled(Feature::CodeMode) {
            return None;
        }

        let exec = ExecContext {
            session: Arc::clone(session),
            turn: Arc::clone(turn),
        };
        let tool_runtime =
            ToolCallRuntime::new(router, Arc::clone(session), Arc::clone(turn), tracker);
        let host = Arc::new(CoreTurnHost { exec, tool_runtime });
        Some(self.inner.start_turn_worker(host))
    }
}

struct CoreTurnHost {
    exec: ExecContext,
    tool_runtime: ToolCallRuntime,
}

#[async_trait::async_trait]
impl CodeModeTurnHost for CoreTurnHost {
    async fn invoke_tool(
        &self,
        tool_name: String,
        input: Option<JsonValue>,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        call_nested_tool(
            self.exec.clone(),
            self.tool_runtime.clone(),
            tool_name,
            input,
            cancellation_token,
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn notify(&self, call_id: String, cell_id: String, text: String) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.exec
            .session
            .inject_response_items(vec![ResponseInputItem::CustomToolCallOutput {
                call_id,
                name: Some(PUBLIC_TOOL_NAME.to_string()),
                output: FunctionCallOutputPayload::from_text(text),
            }])
            .await
            .map_err(|_| {
                format!("failed to inject exec notify message for cell {cell_id}: no active turn")
            })
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
            content_items = truncate_code_mode_result(content_items, max_output_tokens);
            prepend_script_status(&mut content_items, &script_status, started_at.elapsed());
            Ok(FunctionToolOutput::from_content(content_items, Some(true)))
        }
        RuntimeResponse::Terminated { content_items, .. } => {
            let mut content_items = into_function_call_output_content_items(content_items);
            content_items = truncate_code_mode_result(content_items, max_output_tokens);
            prepend_script_status(&mut content_items, &script_status, started_at.elapsed());
            Ok(FunctionToolOutput::from_content(content_items, Some(true)))
        }
        RuntimeResponse::Result {
            content_items,
            stored_values,
            error_text,
            ..
        } => {
            let mut content_items = into_function_call_output_content_items(content_items);
            exec.session
                .services
                .code_mode_service
                .replace_stored_values(stored_values)
                .await;
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

pub(super) async fn build_enabled_tools(
    exec: &ExecContext,
) -> Vec<codex_code_mode::ToolDefinition> {
    let router = build_nested_router(exec).await;
    let specs = router.specs();
    collect_code_mode_tool_definitions(&specs)
}

async fn build_nested_router(exec: &ExecContext) -> ToolRouter {
    let nested_tools_config = exec.turn.tools_config.for_code_mode_nested_tools();
    let listed_mcp_tools = exec
        .session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let parallel_mcp_server_names = exec
        .turn
        .config
        .mcp_servers
        .get()
        .iter()
        .filter_map(|(server_name, server_config)| {
            server_config
                .supports_parallel_tool_calls
                .then_some(server_name.clone())
        })
        .collect::<HashSet<_>>();

    ToolRouter::from_config(
        &nested_tools_config,
        ToolRouterParams {
            mcp_tools: Some(listed_mcp_tools),
            deferred_mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names,
            discoverable_tools: None,
            dynamic_tools: exec.turn.dynamic_tools.as_slice(),
        },
    )
}

async fn call_nested_tool(
    exec: ExecContext,
    tool_runtime: ToolCallRuntime,
    tool_name: String,
    input: Option<JsonValue>,
    cancellation_token: CancellationToken,
) -> Result<JsonValue, FunctionCallError> {
    if tool_name == PUBLIC_TOOL_NAME {
        return Err(FunctionCallError::RespondToModel(format!(
            "{PUBLIC_TOOL_NAME} cannot invoke itself"
        )));
    }

    let payload = if let Some(tool_info) = exec
        .session
        .resolve_mcp_tool_info(&tool_name, /*namespace*/ None)
        .await
    {
        match serialize_function_tool_arguments(&tool_name, input) {
            Ok(raw_arguments) => ToolPayload::Mcp {
                server: tool_info.server_name,
                tool: tool_info.tool.name.to_string(),
                raw_arguments,
            },
            Err(error) => return Err(FunctionCallError::RespondToModel(error)),
        }
    } else {
        match build_nested_tool_payload(tool_runtime.find_spec(&tool_name), &tool_name, input) {
            Ok(payload) => payload,
            Err(error) => return Err(FunctionCallError::RespondToModel(error)),
        }
    };

    let call = ToolCall {
        tool_name: ToolName::plain(tool_name.clone()),
        call_id: format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4()),
        payload,
    };
    let result = tool_runtime
        .handle_tool_call_with_source(call, ToolCallSource::CodeMode, cancellation_token)
        .await?;
    Ok(result.code_mode_result())
}

fn tool_kind_for_spec(spec: &ToolSpec) -> codex_code_mode::CodeModeToolKind {
    if matches!(spec, ToolSpec::Freeform(_)) {
        codex_code_mode::CodeModeToolKind::Freeform
    } else {
        codex_code_mode::CodeModeToolKind::Function
    }
}

fn tool_kind_for_name(
    spec: Option<ToolSpec>,
    tool_name: &str,
) -> Result<codex_code_mode::CodeModeToolKind, String> {
    spec.as_ref()
        .map(tool_kind_for_spec)
        .ok_or_else(|| format!("tool `{tool_name}` is not enabled in {PUBLIC_TOOL_NAME}"))
}

fn build_nested_tool_payload(
    spec: Option<ToolSpec>,
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let actual_kind = tool_kind_for_name(spec, tool_name)?;
    match actual_kind {
        codex_code_mode::CodeModeToolKind::Function => {
            build_function_tool_payload(tool_name, input)
        }
        codex_code_mode::CodeModeToolKind::Freeform => {
            build_freeform_tool_payload(tool_name, input)
        }
    }
}

fn build_function_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let arguments = serialize_function_tool_arguments(tool_name, input)?;
    Ok(ToolPayload::Function { arguments })
}

fn serialize_function_tool_arguments(
    tool_name: &str,
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
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    match input {
        Some(JsonValue::String(input)) => Ok(ToolPayload::Custom { input }),
        _ => Err(format!("tool `{tool_name}` expects a string input")),
    }
}
