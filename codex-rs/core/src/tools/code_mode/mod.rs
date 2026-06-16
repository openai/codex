mod delegate;
mod execute_handler;
pub(crate) mod execute_spec;
mod response_adapter;
mod wait_handler;
pub(crate) mod wait_spec;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CellOutcome;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::RuntimeResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
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
use crate::tools::effective_tool_mode;
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

pub(crate) const PUBLIC_TOOL_NAME: &str = codex_code_mode_protocol::PUBLIC_TOOL_NAME;
pub(crate) const WAIT_TOOL_NAME: &str = codex_code_mode_protocol::WAIT_TOOL_NAME;
pub(crate) const DEFAULT_WAIT_YIELD_TIME_MS: u64 =
    codex_code_mode_protocol::DEFAULT_WAIT_YIELD_TIME_MS;

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
    session: Mutex<Option<Arc<dyn CodeModeSession>>>,
    provider: Arc<dyn CodeModeSessionProvider>,
    dispatch_broker: Arc<CodeModeDispatchBroker>,
    session_init_permit: Semaphore,
    shutting_down: AtomicBool,
}

struct InitialObservationGuard {
    session: Arc<dyn CodeModeSession>,
    dispatch_broker: Arc<CodeModeDispatchBroker>,
    cell_id: Option<CellId>,
    code_cell_trace: codex_rollout_trace::CodeCellTraceContext,
}

impl InitialObservationGuard {
    async fn new(
        service: &CodeModeService,
        cell_id: CellId,
        code_cell_trace: codex_rollout_trace::CodeCellTraceContext,
    ) -> Result<Self, String> {
        let session = service
            .session
            .lock()
            .await
            .clone()
            .ok_or_else(|| "code mode is unavailable".to_string())?;
        Ok(Self {
            session,
            dispatch_broker: Arc::clone(&service.dispatch_broker),
            cell_id: Some(cell_id),
            code_cell_trace,
        })
    }

    fn finish_with_error(&mut self, error_text: &str) {
        let Some(cell_id) = self.cell_id.take() else {
            return;
        };
        let response = RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
            error_text: Some(error_text.to_string()),
        };
        self.code_cell_trace.record_initial_response(&response);
        self.code_cell_trace.record_ended(&response);
        let session = Arc::clone(&self.session);
        let dispatch_broker = Arc::clone(&self.dispatch_broker);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            dispatch_broker.close_cell(&cell_id);
            tracing::warn!("unable to terminate code-mode cell {cell_id}: no Tokio runtime");
            return;
        };
        let _termination_task = runtime.spawn(async move {
            if let Err(error) = session.terminate(cell_id.clone()).await {
                tracing::warn!("failed to terminate code-mode cell {cell_id}: {error}");
            }
            dispatch_broker.close_cell(&cell_id);
        });
    }

    fn disarm(&mut self) {
        self.cell_id = None;
    }
}

impl Drop for InitialObservationGuard {
    fn drop(&mut self) {
        self.finish_with_error("initial code-mode observation cancelled");
    }
}

impl CodeModeService {
    pub(crate) fn new(provider: Arc<dyn CodeModeSessionProvider>) -> Self {
        let dispatch_broker = Arc::new(CodeModeDispatchBroker::new());
        Self {
            session: Mutex::new(None),
            provider,
            dispatch_broker,
            session_init_permit: Semaphore::new(/*permits*/ 1),
            shutting_down: AtomicBool::new(false),
        }
    }

    pub(crate) async fn create_cell(
        &self,
        request: codex_code_mode_protocol::CreateCellRequest,
    ) -> Result<CellId, String> {
        self.session_for_create().await?.create_cell(request).await
    }

    pub(crate) async fn observe(
        &self,
        request: codex_code_mode_protocol::ObserveRequest,
    ) -> Result<CellOutcome, String> {
        match self.current_session().await {
            Some(session) => session.observe(request).await,
            None => Ok(missing_cell_outcome(request.cell_id)),
        }
    }

    pub(crate) async fn terminate(&self, cell_id: CellId) -> Result<CellOutcome, String> {
        match self.current_session().await {
            Some(session) => session.terminate(cell_id).await,
            None => Ok(missing_cell_outcome(cell_id)),
        }
    }

    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        self.shutting_down.store(true, Ordering::Release);
        let _permit = self
            .session_init_permit
            .acquire()
            .await
            .map_err(|_| "code mode session initializer closed".to_string())?;
        let session = self.session.lock().await.clone();
        match session {
            Some(session) => session.shutdown().await,
            None => Ok(()),
        }
    }

    pub(crate) fn mark_cell_ready_for_dispatch(&self, cell_id: &CellId) {
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
        let tool_mode = effective_tool_mode(turn);
        if !matches!(tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly) {
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

    async fn session_for_create(&self) -> Result<Arc<dyn CodeModeSession>, String> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err("code mode session is shutting down".to_string());
        }
        if let Some(session) = self.current_session().await {
            return Ok(session);
        }
        let _permit = self
            .session_init_permit
            .acquire()
            .await
            .map_err(|_| "code mode session initializer closed".to_string())?;
        if self.shutting_down.load(Ordering::Acquire) {
            return Err("code mode session is shutting down".to_string());
        }
        if let Some(session) = self.current_session().await {
            return Ok(session);
        }
        let session = self
            .provider
            .create_session(self.dispatch_broker.clone())
            .await?;
        if self.shutting_down.load(Ordering::Acquire) {
            let _ = session.shutdown().await;
            return Err("code mode session is shutting down".to_string());
        }
        *self.session.lock().await = Some(Arc::clone(&session));
        Ok(session)
    }

    async fn current_session(&self) -> Option<Arc<dyn CodeModeSession>> {
        self.session
            .lock()
            .await
            .as_ref()
            .filter(|session| session.is_alive())
            .cloned()
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

fn missing_cell_outcome(cell_id: CellId) -> CellOutcome {
    CellOutcome::MissingCell(RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    })
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
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::CodeModeDispatchBroker;
    use super::CodeModeService;
    use super::InitialObservationGuard;
    use super::build_nested_tool_payload;
    use super::truncate_code_mode_result;
    use crate::tools::context::ToolPayload;
    use codex_code_mode_protocol::CellId;
    use codex_code_mode_protocol::CellOutcome;
    use codex_code_mode_protocol::CodeModeSession;
    use codex_code_mode_protocol::CodeModeSessionDelegate;
    use codex_code_mode_protocol::CodeModeSessionProvider;
    use codex_code_mode_protocol::CodeModeSessionProviderFuture;
    use codex_code_mode_protocol::CodeModeSessionResultFuture;
    use codex_code_mode_protocol::CodeModeToolKind;
    use codex_code_mode_protocol::CreateCellRequest;
    use codex_code_mode_protocol::ObserveRequest;
    use codex_code_mode_protocol::RuntimeResponse;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_tools::ToolName;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    struct RecordingCodeModeSession {
        terminated_cells: Mutex<Vec<CellId>>,
        termination: tokio::sync::Notify,
    }

    impl CodeModeSession for RecordingCodeModeSession {
        fn is_alive(&self) -> bool {
            true
        }

        fn create_cell<'a>(
            &'a self,
            _request: CreateCellRequest,
        ) -> CodeModeSessionResultFuture<'a, CellId> {
            Box::pin(async { panic!("unexpected cell creation") })
        }

        fn observe<'a>(
            &'a self,
            _request: ObserveRequest,
        ) -> CodeModeSessionResultFuture<'a, CellOutcome> {
            Box::pin(async { panic!("unexpected cell observation") })
        }

        fn terminate<'a>(
            &'a self,
            cell_id: CellId,
        ) -> CodeModeSessionResultFuture<'a, CellOutcome> {
            Box::pin(async move {
                self.terminated_cells
                    .lock()
                    .expect("terminated-cell lock should not be poisoned")
                    .push(cell_id.clone());
                self.termination.notify_one();
                Ok(CellOutcome::MissingCell(RuntimeResponse::Result {
                    cell_id,
                    content_items: Vec::new(),
                    error_text: Some("terminated by test".to_string()),
                }))
            })
        }

        fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

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

    #[test]
    fn truncated_text_output_starts_with_warning() {
        let items = vec![FunctionCallOutputContentItem::InputText {
            text: "0123456789012345678901234567890123456789".to_string(),
        }];

        assert_eq!(
            truncate_code_mode_result(items, Some(5)),
            vec![FunctionCallOutputContentItem::InputText {
                text: concat!(
                    "Warning: truncated output (original token count: 10)\n",
                    "Total output lines: 1\n\n",
                    "0123456789…5 tokens truncated…0123456789"
                )
                .to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn dropping_initial_observation_guard_terminates_the_cell() {
        let session = Arc::new(RecordingCodeModeSession {
            terminated_cells: Mutex::new(Vec::new()),
            termination: tokio::sync::Notify::new(),
        });
        let session_trait: Arc<dyn CodeModeSession> = session.clone();
        let dispatch_broker = Arc::new(CodeModeDispatchBroker::new());
        let cell_id = CellId::new("cell-1".to_string());
        dispatch_broker.mark_cell_ready_for_dispatch(&cell_id);
        let code_cell_trace = codex_rollout_trace::ThreadTraceContext::disabled()
            .code_cell_trace_context("turn-1", cell_id.as_str());

        drop(InitialObservationGuard {
            session: session_trait,
            dispatch_broker,
            cell_id: Some(cell_id.clone()),
            code_cell_trace,
        });

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            session.termination.notified(),
        )
        .await
        .expect("guard should schedule cell termination");
        assert_eq!(
            *session
                .terminated_cells
                .lock()
                .expect("terminated-cell lock should not be poisoned"),
            vec![cell_id]
        );
    }

    struct RecoveringSessionProvider {
        sessions_created: AtomicUsize,
    }

    impl CodeModeSessionProvider for RecoveringSessionProvider {
        fn create_session<'a>(
            &'a self,
            _delegate: Arc<dyn CodeModeSessionDelegate>,
        ) -> CodeModeSessionProviderFuture<'a> {
            let generation = self.sessions_created.fetch_add(1, Ordering::Relaxed) + 1;
            Box::pin(async move {
                let session: Arc<dyn CodeModeSession> = Arc::new(RecoveringSession {
                    generation,
                    alive: AtomicBool::new(true),
                });
                Ok(session)
            })
        }
    }

    struct RecoveringSession {
        generation: usize,
        alive: AtomicBool,
    }

    impl CodeModeSession for RecoveringSession {
        fn is_alive(&self) -> bool {
            self.alive.load(Ordering::Acquire)
        }

        fn create_cell<'a>(
            &'a self,
            _request: CreateCellRequest,
        ) -> CodeModeSessionResultFuture<'a, CellId> {
            Box::pin(async move {
                if self.generation == 1 {
                    self.alive.store(false, Ordering::Release);
                    return Err("host crashed".to_string());
                }
                Ok(CellId::new(format!("cell-{}", self.generation)))
            })
        }

        fn observe<'a>(
            &'a self,
            request: ObserveRequest,
        ) -> CodeModeSessionResultFuture<'a, CellOutcome> {
            Box::pin(async move {
                Ok(CellOutcome::LiveCell(RuntimeResponse::Result {
                    cell_id: request.cell_id,
                    content_items: Vec::new(),
                    error_text: None,
                }))
            })
        }

        fn terminate<'a>(
            &'a self,
            _cell_id: CellId,
        ) -> CodeModeSessionResultFuture<'a, CellOutcome> {
            Box::pin(async { panic!("terminate should not be sent to a failed session") })
        }

        fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    fn create_cell_request() -> CreateCellRequest {
        CreateCellRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('hello')".to_string(),
        }
    }

    #[tokio::test]
    async fn create_replaces_failed_session_but_observe_does_not() {
        let provider = Arc::new(RecoveringSessionProvider {
            sessions_created: AtomicUsize::new(0),
        });
        let provider_trait: Arc<dyn CodeModeSessionProvider> = provider.clone();
        let service = CodeModeService::new(provider_trait);

        assert_eq!(
            service
                .create_cell(create_cell_request())
                .await
                .err()
                .as_deref(),
            Some("host crashed")
        );
        assert_eq!(provider.sessions_created.load(Ordering::Relaxed), 1);

        assert_eq!(
            service
                .observe(ObserveRequest {
                    cell_id: CellId::new("1".to_string()),
                    yield_time_ms: 1,
                })
                .await,
            Ok(CellOutcome::MissingCell(RuntimeResponse::Result {
                cell_id: CellId::new("1".to_string()),
                content_items: Vec::new(),
                error_text: Some("exec cell 1 not found".to_string()),
            }))
        );
        assert_eq!(provider.sessions_created.load(Ordering::Relaxed), 1);

        assert_eq!(
            service.terminate(CellId::new("1".to_string())).await,
            Ok(CellOutcome::MissingCell(RuntimeResponse::Result {
                cell_id: CellId::new("1".to_string()),
                content_items: Vec::new(),
                error_text: Some("exec cell 1 not found".to_string()),
            }))
        );
        assert_eq!(provider.sessions_created.load(Ordering::Relaxed), 1);

        let cell_id = service.create_cell(create_cell_request()).await.unwrap();
        assert_eq!(cell_id, CellId::new("cell-2".to_string()));
        assert_eq!(
            service
                .observe(ObserveRequest {
                    cell_id: cell_id.clone(),
                    yield_time_ms: 1,
                })
                .await,
            Ok(CellOutcome::LiveCell(RuntimeResponse::Result {
                cell_id,
                content_items: Vec::new(),
                error_text: None,
            }))
        );
        assert_eq!(provider.sessions_created.load(Ordering::Relaxed), 2);
    }
}
