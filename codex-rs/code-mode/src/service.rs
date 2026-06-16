use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::DEFAULT_EXEC_YIELD_TIME_MS;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::ExecuteToPendingOutcome;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::WaitToPendingOutcome;
use codex_code_mode_protocol::WaitToPendingRequest;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::runtime::PendingRuntimeMode;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeControlCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::spawn_runtime;

pub struct NoopCodeModeSessionDelegate;

impl CodeModeSessionDelegate for NoopCodeModeSessionDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            cancellation_token.cancelled().await;
            Err("code mode nested tools are unavailable".to_string())
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

#[derive(Default)]
pub struct InProcessCodeModeSessionProvider;

impl CodeModeSessionProvider for InProcessCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            let session: Arc<dyn CodeModeSession> =
                Arc::new(CodeModeService::with_delegate(delegate));
            Ok(session)
        })
    }
}

#[derive(Clone)]
struct CellHandle {
    control_tx: mpsc::UnboundedSender<CellControlCommand>,
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    cancellation_token: CancellationToken,
    termination_requested: Arc<AtomicBool>,
}

struct Inner {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, CellHandle>>,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    shutting_down: AtomicBool,
    next_cell_id: AtomicU64,
}

pub struct CodeModeService {
    inner: Arc<Inner>,
}

impl CodeModeService {
    pub fn new() -> Self {
        Self::with_delegate(Arc::new(NoopCodeModeSessionDelegate))
    }

    pub fn with_delegate(delegate: Arc<dyn CodeModeSessionDelegate>) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                delegate,
                shutting_down: AtomicBool::new(false),
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    fn allocate_cell_id(&self) -> CellId {
        CellId::new(
            self.inner
                .next_cell_id
                .fetch_add(1, Ordering::Relaxed)
                .to_string(),
        )
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<StartedCell, String> {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err("code mode session is shutting down".to_string());
        }
        let initial_yield_time_ms = request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS);
        let (response_tx, response_rx) = oneshot::channel();
        let cell_id = self.allocate_cell_id();
        self.start_cell(
            cell_id.clone(),
            request,
            CellResponseSender::Runtime(response_tx),
            Some(initial_yield_time_ms),
            PendingRuntimeMode::Continue,
        )
        .await?;

        Ok(StartedCell::from_result_receiver(cell_id, response_rx))
    }

    pub async fn execute_to_pending(
        &self,
        request: ExecuteRequest,
    ) -> Result<ExecuteToPendingOutcome, String> {
        let (response_tx, response_rx) = oneshot::channel();
        let cell_id = self.allocate_cell_id();
        self.start_cell(
            cell_id,
            request,
            CellResponseSender::ExecuteToPending(response_tx),
            /*initial_yield_time_ms*/ None,
            PendingRuntimeMode::PauseUntilResumed,
        )
        .await?;

        response_rx
            .await
            .map_err(|_| "exec runtime ended unexpectedly".to_string())?
    }

    async fn start_cell(
        &self,
        cell_id: CellId,
        request: ExecuteRequest,
        initial_response_tx: CellResponseSender,
        initial_yield_time_ms: Option<u64>,
        pending_mode: PendingRuntimeMode,
    ) -> Result<(), String> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let stored_values = self.inner.stored_values.lock().await.clone();
        let cancellation_token = CancellationToken::new();
        let (runtime_tx, runtime_control_tx, runtime_terminate_handle) = {
            let mut cells = self.inner.cells.lock().await;
            if self.inner.shutting_down.load(Ordering::Acquire) {
                return Err("code mode session is shutting down".to_string());
            }
            if cells.contains_key(&cell_id) {
                return Err(format!("exec cell {cell_id} already exists"));
            }

            let (runtime_tx, runtime_control_tx, runtime_terminate_handle) =
                spawn_runtime(stored_values, request, event_tx, pending_mode)?;

            cells.insert(
                cell_id.clone(),
                CellHandle {
                    control_tx,
                    runtime_tx: runtime_tx.clone(),
                    cancellation_token: cancellation_token.clone(),
                    termination_requested: Arc::new(AtomicBool::new(false)),
                },
            );
            (runtime_tx, runtime_control_tx, runtime_terminate_handle)
        };

        tokio::spawn(run_cell_control(
            Arc::clone(&self.inner),
            CellControlContext {
                cell_id,
                runtime_tx,
                runtime_control_tx,
                pending_mode,
                runtime_terminate_handle,
                cancellation_token,
            },
            event_rx,
            control_rx,
            initial_response_tx,
            initial_yield_time_ms,
        ));

        Ok(())
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<WaitOutcome, String> {
        self.begin_wait(request).await.await
    }

    async fn begin_wait(
        &self,
        request: WaitRequest,
    ) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
        let WaitRequest {
            cell_id,
            yield_time_ms,
        } = request;
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return missing_wait(cell_id);
        };
        let (response_tx, response_rx) = oneshot::channel();
        let control_message = CellControlCommand::Poll {
            yield_time_ms,
            response_tx,
        };
        if handle.control_tx.send(control_message).is_err() {
            return missing_wait(cell_id);
        }
        wait_for_response(cell_id, response_rx)
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id)));
        };
        if handle
            .termination_requested
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return Err(already_terminating_error(&cell_id));
        }
        let (response_tx, response_rx) = oneshot::channel();
        if handle
            .control_tx
            .send(CellControlCommand::Terminate { response_tx })
            .is_err()
        {
            handle.termination_requested.store(false, Ordering::Relaxed);
            return Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id)));
        }
        match response_rx.await {
            Ok(Ok(response)) => Ok(WaitOutcome::LiveCell(response)),
            Ok(Err(error_text)) => Err(error_text),
            Err(_) => Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id))),
        }
    }

    pub async fn wait_to_pending(
        &self,
        request: WaitToPendingRequest,
    ) -> Result<WaitToPendingOutcome, String> {
        let cell_id = request.cell_id;
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                cell_id,
            )));
        };
        let (response_tx, response_rx) = oneshot::channel();
        if handle
            .control_tx
            .send(CellControlCommand::PollToPending { response_tx })
            .is_err()
        {
            return Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                cell_id,
            )));
        }
        match response_rx.await {
            Ok(Ok(response)) => Ok(WaitToPendingOutcome::LiveCell(response)),
            Ok(Err(error_text)) => Err(error_text),
            Err(_) => Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                cell_id,
            ))),
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.inner.shutting_down.store(true, Ordering::Release);
        let handles = self
            .inner
            .cells
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handle in handles {
            handle.cancellation_token.cancel();
            let (response_tx, _response_rx) = oneshot::channel();
            let _ = handle
                .control_tx
                .send(CellControlCommand::Terminate { response_tx });
            let _ = handle.runtime_tx.send(RuntimeCommand::Terminate);
        }
        while !self.inner.cells.lock().await.is_empty() {
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CodeModeService {
    fn drop(&mut self) {
        self.inner.shutting_down.store(true, Ordering::Release);
        if let Ok(cells) = self.inner.cells.try_lock() {
            for handle in cells.values() {
                handle.cancellation_token.cancel();
                let (response_tx, _response_rx) = oneshot::channel();
                let _ = handle
                    .control_tx
                    .send(CellControlCommand::Terminate { response_tx });
                let _ = handle.runtime_tx.send(RuntimeCommand::Terminate);
            }
        }
    }
}

impl CodeModeSession for CodeModeService {
    fn is_alive(&self) -> bool {
        !self.inner.shutting_down.load(Ordering::Acquire)
    }

    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(CodeModeService::execute(self, request))
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(CodeModeService::wait(self, request))
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(CodeModeService::terminate(self, cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(CodeModeService::shutdown(self))
    }
}

enum CellControlCommand {
    Poll {
        yield_time_ms: u64,
        response_tx: oneshot::Sender<Result<RuntimeResponse, String>>,
    },
    PollToPending {
        response_tx: oneshot::Sender<Result<ExecuteToPendingOutcome, String>>,
    },
    Terminate {
        response_tx: oneshot::Sender<Result<RuntimeResponse, String>>,
    },
}

enum CellResponseSender {
    Runtime(oneshot::Sender<Result<RuntimeResponse, String>>),
    ExecuteToPending(oneshot::Sender<Result<ExecuteToPendingOutcome, String>>),
}

struct PendingResult {
    content_items: Vec<FunctionCallOutputContentItem>,
    error_text: Option<String>,
}

struct CellControlContext {
    cell_id: CellId,
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    runtime_control_tx: std::sync::mpsc::Sender<RuntimeControlCommand>,
    pending_mode: PendingRuntimeMode,
    runtime_terminate_handle: v8::IsolateHandle,
    cancellation_token: CancellationToken,
}

fn missing_cell_response(cell_id: CellId) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    }
}

fn missing_wait(cell_id: CellId) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
    Box::pin(async move { Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id))) })
}

fn wait_for_response(
    cell_id: CellId,
    response_rx: oneshot::Receiver<Result<RuntimeResponse, String>>,
) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
    Box::pin(async move {
        match response_rx.await {
            Ok(Ok(response)) => Ok(WaitOutcome::LiveCell(response)),
            Ok(Err(error_text)) => Err(error_text),
            Err(_) => Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id))),
        }
    })
}

fn busy_observer_error(cell_id: &CellId) -> String {
    format!("exec cell {cell_id} already has an active observer")
}

fn already_terminating_error(cell_id: &CellId) -> String {
    format!("exec cell {cell_id} is already terminating")
}

fn pending_result_response(cell_id: &CellId, result: PendingResult) -> RuntimeResponse {
    RuntimeResponse::Result {
        cell_id: cell_id.clone(),
        content_items: result.content_items,
        error_text: result.error_text,
    }
}

fn send_terminal_response(response_tx: CellResponseSender, response: RuntimeResponse) {
    match response_tx {
        CellResponseSender::Runtime(response_tx) => {
            let _ = response_tx.send(Ok(response));
        }
        CellResponseSender::ExecuteToPending(response_tx) => {
            let _ = response_tx.send(Ok(ExecuteToPendingOutcome::Completed(response)));
        }
    }
}

fn send_termination_responses(
    response_tx: Option<CellResponseSender>,
    termination_response_tx: Option<oneshot::Sender<Result<RuntimeResponse, String>>>,
    response: RuntimeResponse,
) {
    if let Some(response_tx) = response_tx {
        send_terminal_response(response_tx, response.clone());
    }
    if let Some(termination_response_tx) = termination_response_tx {
        let _ = termination_response_tx.send(Ok(response));
    }
}

fn send_or_buffer_result(
    cell_id: &CellId,
    result: PendingResult,
    response_tx: &mut Option<CellResponseSender>,
    pending_result: &mut Option<PendingResult>,
) -> bool {
    if let Some(response_tx) = response_tx.take() {
        let response = pending_result_response(cell_id, result);
        send_terminal_response(response_tx, response);
        return true;
    }

    *pending_result = Some(result);
    false
}

fn send_yield_response(
    cell_id: &CellId,
    content_items: &mut Vec<FunctionCallOutputContentItem>,
    response_tx: &mut Option<CellResponseSender>,
) {
    let Some(current_response_tx) = response_tx.take() else {
        return;
    };
    match current_response_tx {
        CellResponseSender::Runtime(response_tx) => {
            let _ = response_tx.send(Ok(RuntimeResponse::Yielded {
                cell_id: cell_id.clone(),
                content_items: std::mem::take(content_items),
            }));
        }
        CellResponseSender::ExecuteToPending(execute_to_pending_tx) => {
            *response_tx = Some(CellResponseSender::ExecuteToPending(execute_to_pending_tx));
        }
    }
}

async fn run_cell_control(
    inner: Arc<Inner>,
    context: CellControlContext,
    mut event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    mut control_rx: mpsc::UnboundedReceiver<CellControlCommand>,
    initial_response_tx: CellResponseSender,
    initial_yield_time_ms: Option<u64>,
) {
    let CellControlContext {
        cell_id,
        runtime_tx,
        runtime_control_tx,
        pending_mode,
        runtime_terminate_handle,
        cancellation_token,
    } = context;
    let mut content_items = Vec::new();
    let mut pending_tool_call_ids = Vec::new();
    let mut pending_result: Option<PendingResult> = None;
    let mut response_tx = Some(initial_response_tx);
    let mut termination_response_tx = None;
    let mut termination_requested = false;
    let mut runtime_closed = false;
    let mut yield_timer: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
    let mut notification_tasks = JoinSet::new();
    let mut tool_tasks = JoinSet::new();

    loop {
        let yield_deadline_elapsed = yield_timer
            .as_ref()
            .is_some_and(|yield_timer| yield_timer.deadline() <= tokio::time::Instant::now());
        tokio::select! {
            biased;
            maybe_command = control_rx.recv() => {
                let Some(command) = maybe_command else {
                    break;
                };
                match command {
                    CellControlCommand::Poll {
                        yield_time_ms,
                        response_tx: next_response_tx,
                    } => {
                        if let Some(result) = pending_result.take() {
                            let _ = next_response_tx.send(Ok(pending_result_response(&cell_id, result)));
                            break;
                        }
                        if response_tx.is_some() || termination_response_tx.is_some() {
                            let _ = next_response_tx.send(Err(busy_observer_error(&cell_id)));
                            continue;
                        }
                        response_tx = Some(CellResponseSender::Runtime(next_response_tx));
                        yield_timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(yield_time_ms))));
                        resume_paused_runtime(&runtime_control_tx, pending_mode);
                    }
                    CellControlCommand::PollToPending {
                        response_tx: next_response_tx,
                    } => {
                        if let Some(result) = pending_result.take() {
                            let response = pending_result_response(&cell_id, result);
                            let _ = next_response_tx
                                .send(Ok(ExecuteToPendingOutcome::Completed(response)));
                            break;
                        }
                        if response_tx.is_some() || termination_response_tx.is_some() {
                            let _ = next_response_tx.send(Err(busy_observer_error(&cell_id)));
                            continue;
                        }
                        response_tx =
                            Some(CellResponseSender::ExecuteToPending(next_response_tx));
                        yield_timer = None;
                        resume_paused_runtime(&runtime_control_tx, pending_mode);
                    }
                    CellControlCommand::Terminate { response_tx: next_response_tx } => {
                        if let Some(result) = pending_result.take() {
                            let _ = next_response_tx.send(Ok(pending_result_response(&cell_id, result)));
                            break;
                        }

                        if termination_response_tx.is_some() {
                            let _ = next_response_tx.send(Err(already_terminating_error(&cell_id)));
                            continue;
                        }

                        termination_response_tx = Some(next_response_tx);
                        termination_requested = true;
                        cancellation_token.cancel();
                        yield_timer = None;
                        let _ = runtime_tx.send(RuntimeCommand::Terminate);
                        terminate_paused_runtime(&runtime_control_tx, pending_mode);
                        let _ = runtime_terminate_handle.terminate_execution();
                        if runtime_closed {
                            finish_callbacks(
                                &cancellation_token,
                                &mut notification_tasks,
                                &mut tool_tasks,
                                CallbackCompletion::Cancel,
                            ).await;
                            let response = RuntimeResponse::Terminated {
                                cell_id: cell_id.clone(),
                                content_items: std::mem::take(&mut content_items),
                            };
                            send_termination_responses(
                                response_tx.take(),
                                termination_response_tx.take(),
                                response,
                            );
                            break;
                        } else {
                            continue;
                        }
                    }
                }
            }
            _ = async {
                if let Some(yield_timer) = yield_timer.as_mut() {
                    yield_timer.await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                yield_timer = None;
                send_yield_response(&cell_id, &mut content_items, &mut response_tx);
            }
            maybe_event = async {
                if runtime_closed {
                    std::future::pending::<Option<RuntimeEvent>>().await
                } else {
                    event_rx.recv().await
                }
            }, if !yield_deadline_elapsed => {
                let Some(event) = maybe_event else {
                    runtime_closed = true;
                    if termination_requested {
                        finish_callbacks(
                            &cancellation_token,
                            &mut notification_tasks,
                            &mut tool_tasks,
                            CallbackCompletion::Cancel,
                        ).await;
                        let response = RuntimeResponse::Terminated {
                            cell_id: cell_id.clone(),
                            content_items: std::mem::take(&mut content_items),
                        };
                        send_termination_responses(
                            response_tx.take(),
                            termination_response_tx.take(),
                            response,
                        );
                        break;
                    }
                    if pending_result.is_none() {
                        let result = PendingResult {
                            content_items: std::mem::take(&mut content_items),
                            error_text: Some("exec runtime ended unexpectedly".to_string()),
                        };
                        if send_or_buffer_result(
                            &cell_id,
                            result,
                            &mut response_tx,
                            &mut pending_result,
                        ) {
                            break;
                        }
                    }
                    continue;
                };
                match event {
                    RuntimeEvent::Started => {
                        yield_timer = initial_yield_time_ms.map(|initial_yield_time_ms| {
                            Box::pin(tokio::time::sleep(Duration::from_millis(initial_yield_time_ms)))
                        });
                    }
                    RuntimeEvent::Pending => {
                        if let Some(current_response_tx) = response_tx.take() {
                            match current_response_tx {
                                CellResponseSender::Runtime(runtime_response_tx) => {
                                    response_tx =
                                        Some(CellResponseSender::Runtime(runtime_response_tx));
                                }
                                CellResponseSender::ExecuteToPending(response_tx) => {
                                    let _ = response_tx.send(Ok(ExecuteToPendingOutcome::Pending {
                                        cell_id: cell_id.clone(),
                                        content_items: std::mem::take(&mut content_items),
                                        pending_tool_call_ids: std::mem::take(
                                            &mut pending_tool_call_ids,
                                        ),
                                    }));
                                }
                            }
                        }
                    }
                    RuntimeEvent::ContentItem(item) => {
                        content_items.push(item);
                    }
                    RuntimeEvent::YieldRequested => {
                        yield_timer = None;
                        send_yield_response(&cell_id, &mut content_items, &mut response_tx);
                    }
                    RuntimeEvent::Notify { call_id, text } => {
                        let delegate = Arc::clone(&inner.delegate);
                        let cell_id = cell_id.clone();
                        let cancellation_token = cancellation_token.child_token();
                        notification_tasks.spawn(async move {
                            if let Err(err) = delegate
                                .notify(call_id, cell_id.clone(), text, cancellation_token)
                                .await
                            {
                                warn!(
                                    "failed to deliver code mode notification for cell {cell_id}: {err}"
                                );
                            }
                        });
                    }
                    RuntimeEvent::ToolCall {
                        id,
                        name,
                        kind,
                        input,
                    } => {
                        if pending_mode == PendingRuntimeMode::PauseUntilResumed {
                            pending_tool_call_ids.push(id.clone());
                        }
                        let tool_call = CodeModeNestedToolCall {
                            cell_id: cell_id.clone(),
                            runtime_tool_call_id: id.clone(),
                            tool_name: name,
                            tool_kind: kind,
                            input,
                        };
                        let delegate = Arc::clone(&inner.delegate);
                        let runtime_tx = runtime_tx.clone();
                        let cancellation_token = cancellation_token.child_token();
                        tool_tasks.spawn(async move {
                            let response = delegate.invoke_tool(tool_call, cancellation_token).await;
                            let command = match response {
                                Ok(result) => RuntimeCommand::ToolResponse { id, result },
                                Err(error_text) => RuntimeCommand::ToolError { id, error_text },
                            };
                            let _ = runtime_tx.send(command);
                        });
                    }
                    RuntimeEvent::Result {
                        stored_value_writes,
                        error_text,
                    } => {
                        yield_timer = None;
                        if termination_requested {
                            finish_callbacks(
                                &cancellation_token,
                                &mut notification_tasks,
                                &mut tool_tasks,
                                CallbackCompletion::Cancel,
                            ).await;
                            let response = RuntimeResponse::Terminated {
                                cell_id: cell_id.clone(),
                                content_items: std::mem::take(&mut content_items),
                            };
                            send_termination_responses(
                                response_tx.take(),
                                termination_response_tx.take(),
                                response,
                            );
                            break;
                        }
                        finish_callbacks(
                            &cancellation_token,
                            &mut notification_tasks,
                            &mut tool_tasks,
                            CallbackCompletion::DrainNotifications,
                        ).await;
                        inner
                            .stored_values
                            .lock()
                            .await
                            .extend(stored_value_writes);
                        let result = PendingResult {
                            content_items: std::mem::take(&mut content_items),
                            error_text,
                        };
                        if send_or_buffer_result(
                            &cell_id,
                            result,
                            &mut response_tx,
                            &mut pending_result,
                        ) {
                            break;
                        }
                    }
                }
            }
            task_result = notification_tasks.join_next(), if !notification_tasks.is_empty() => {
                if let Some(Err(err)) = task_result
                    && !err.is_cancelled()
                {
                    warn!("code mode notification task failed: {err}");
                }
            }
            task_result = tool_tasks.join_next(), if !tool_tasks.is_empty() => {
                if let Some(Err(err)) = task_result
                    && !err.is_cancelled()
                {
                    warn!("code mode tool task failed: {err}");
                }
            }
        }
    }

    let _ = runtime_tx.send(RuntimeCommand::Terminate);
    cancellation_token.cancel();
    finish_callbacks(
        &cancellation_token,
        &mut notification_tasks,
        &mut tool_tasks,
        CallbackCompletion::Cancel,
    )
    .await;
    terminate_paused_runtime(&runtime_control_tx, pending_mode);
    inner.cells.lock().await.remove(&cell_id);
    inner.delegate.cell_closed(&cell_id);
}

#[derive(Clone, Copy)]
enum CallbackCompletion {
    DrainNotifications,
    Cancel,
}

async fn finish_callbacks(
    cancellation_token: &CancellationToken,
    notification_tasks: &mut JoinSet<()>,
    tool_tasks: &mut JoinSet<()>,
    completion: CallbackCompletion,
) {
    if matches!(completion, CallbackCompletion::Cancel) {
        cancellation_token.cancel();
    }
    drain_tasks(notification_tasks, "notification").await;
    cancellation_token.cancel();
    drain_tasks(tool_tasks, "tool").await;
}

async fn drain_tasks(tasks: &mut JoinSet<()>, description: &str) {
    while let Some(result) = tasks.join_next().await {
        if let Err(err) = result
            && !err.is_cancelled()
        {
            warn!("code mode {description} task failed: {err}");
        }
    }
}

fn resume_paused_runtime(
    runtime_control_tx: &std::sync::mpsc::Sender<RuntimeControlCommand>,
    pending_mode: PendingRuntimeMode,
) {
    if pending_mode == PendingRuntimeMode::PauseUntilResumed {
        let _ = runtime_control_tx.send(RuntimeControlCommand::Resume);
    }
}

fn terminate_paused_runtime(
    runtime_control_tx: &std::sync::mpsc::Sender<RuntimeControlCommand>,
    pending_mode: PendingRuntimeMode,
) {
    if pending_mode == PendingRuntimeMode::PauseUntilResumed {
        let _ = runtime_control_tx.send(RuntimeControlCommand::Terminate);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use codex_protocol::ToolName;
    use pretty_assertions::assert_eq;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    use super::CellControlCommand;
    use super::CellControlContext;
    use super::CellId;
    use super::CellResponseSender;
    use super::CodeModeService;
    use super::Inner;
    use super::NoopCodeModeSessionDelegate;
    use super::PendingRuntimeMode;
    use super::RuntimeCommand;
    use super::RuntimeResponse;
    use super::WaitOutcome;
    use super::WaitRequest;
    use super::WaitToPendingOutcome;
    use super::WaitToPendingRequest;
    use super::run_cell_control;
    use crate::CodeModeToolKind;
    use crate::ExecuteRequest;
    use crate::ExecuteToPendingOutcome;
    use crate::FunctionCallOutputContentItem;
    use crate::ToolDefinition;
    use crate::runtime::RuntimeEvent;
    use crate::runtime::spawn_runtime;

    fn execute_request(source: &str) -> ExecuteRequest {
        ExecuteRequest {
            tool_call_id: "call_1".to_string(),
            enabled_tools: Vec::new(),
            source: source.to_string(),
            yield_time_ms: Some(1),
            max_output_tokens: None,
        }
    }

    fn cell_id(value: &str) -> CellId {
        CellId::new(value.to_string())
    }

    async fn execute(service: &CodeModeService, request: ExecuteRequest) -> RuntimeResponse {
        service
            .execute(request)
            .await
            .unwrap()
            .initial_response()
            .await
            .unwrap()
    }

    fn test_inner() -> Arc<Inner> {
        Arc::new(Inner {
            stored_values: Mutex::new(HashMap::new()),
            cells: Mutex::new(HashMap::new()),
            delegate: Arc::new(NoopCodeModeSessionDelegate),
            shutting_down: std::sync::atomic::AtomicBool::new(false),
            next_cell_id: AtomicU64::new(1),
        })
    }

    #[tokio::test]
    async fn synchronous_exit_returns_successfully() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"text("before"); exit(); text("after");"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn stored_values_are_shared_between_cells_but_not_sessions() {
        let first_session = CodeModeService::new();
        let second_session = CodeModeService::new();

        let write_response = execute(
            &first_session,
            ExecuteRequest {
                source: r#"store("key", "visible");"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        let same_session = execute(
            &first_session,
            ExecuteRequest {
                source: r#"text(String(load("key")));"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;
        let other_session = execute(
            &second_session,
            ExecuteRequest {
                source: r#"text(String(load("key")));"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            write_response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: None,
            }
        );
        assert_eq!(
            same_session,
            RuntimeResponse::Result {
                cell_id: cell_id("2"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "visible".to_string(),
                }],
                error_text: None,
            }
        );
        assert_eq!(
            other_session,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "undefined".to_string(),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn shutdown_interrupts_cpu_bound_cells() {
        let service = CodeModeService::new();

        let cell = service
            .execute(ExecuteRequest {
                source: "while (true) {}".to_string(),
                ..execute_request("")
            })
            .await
            .unwrap();
        assert_eq!(
            cell.initial_response().await.unwrap(),
            RuntimeResponse::Yielded {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
            }
        );

        tokio::time::timeout(Duration::from_secs(1), service.shutdown())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn start_cell_rejects_new_cell_after_shutdown_begins() {
        let service = CodeModeService::new();
        service.inner.shutting_down.store(true, Ordering::Release);
        let (response_tx, _response_rx) = oneshot::channel();

        let error = service
            .start_cell(
                cell_id("late-cell"),
                execute_request(""),
                CellResponseSender::Runtime(response_tx),
                Some(/*initial_yield_time_ms*/ 1),
                PendingRuntimeMode::Continue,
            )
            .await
            .unwrap_err();

        assert_eq!(error, "code mode session is shutting down".to_string());
        assert!(service.inner.cells.lock().await.is_empty());
    }

    #[tokio::test]
    async fn execute_to_pending_returns_completed_for_synchronous_results() {
        let service = CodeModeService::new();

        let response = service
            .execute_to_pending(ExecuteRequest {
                source: r#"text("done");"#.to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            ExecuteToPendingOutcome::Completed(RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                error_text: None,
            })
        );
    }

    #[tokio::test]
    async fn execute_to_pending_returns_once_the_runtime_is_quiescent() {
        let service = CodeModeService::new();

        let response = tokio::time::timeout(
            Duration::from_secs(1),
            service.execute_to_pending(ExecuteRequest {
                source: r#"text("before"); await new Promise(() => {});"#.to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            }),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            response,
            ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                }],
                pending_tool_call_ids: Vec::new(),
            }
        );

        let termination = service.terminate(cell_id("1")).await.unwrap();

        assert_eq!(
            termination,
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
            })
        );
    }

    #[tokio::test]
    async fn execute_to_pending_identifies_tool_calls_in_paused_frontier() {
        let service = CodeModeService::new();

        let response = service
            .execute_to_pending(ExecuteRequest {
                enabled_tools: vec![ToolDefinition {
                    name: "echo".to_string(),
                    tool_name: ToolName::plain("echo"),
                    description: String::new(),
                    kind: CodeModeToolKind::Function,
                    input_schema: None,
                    output_schema: None,
                }],
                source: r#"
await Promise.all([
  tools.echo({ value: "first" }),
  tools.echo({ value: "second" }),
]);
"#
                .to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
            }
        );

        let termination = service.terminate(cell_id("1")).await.unwrap();

        assert_eq!(
            termination,
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
            })
        );
    }

    #[tokio::test]
    async fn execute_to_pending_excludes_delayed_timeout_tool_calls_until_wait() {
        let service = CodeModeService::new();

        let initial_response = service
            .execute_to_pending(ExecuteRequest {
                enabled_tools: vec![ToolDefinition {
                    name: "echo".to_string(),
                    tool_name: ToolName::plain("echo"),
                    description: String::new(),
                    kind: CodeModeToolKind::Function,
                    input_schema: None,
                    output_schema: None,
                }],
                source: r#"
setTimeout(() => {
  tools.echo({ value: "delayed" });
}, 1000);
await Promise.all([
  tools.echo({ value: "second" }),
  tools.echo({ value: "third" }),
]);
"#
                .to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            initial_response,
            ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                pending_tool_call_ids: vec!["tool-1".to_string(), "tool-2".to_string()],
            }
        );

        let runtime_tx = service
            .inner
            .cells
            .lock()
            .await
            .get(&cell_id("1"))
            .unwrap()
            .runtime_tx
            .clone();
        runtime_tx
            .send(RuntimeCommand::TimeoutFired { id: 1 })
            .unwrap();

        let resumed_response = tokio::time::timeout(
            Duration::from_secs(1),
            service.wait_to_pending(WaitToPendingRequest {
                cell_id: cell_id("1"),
            }),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            resumed_response,
            WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                pending_tool_call_ids: vec!["tool-3".to_string()],
            })
        );

        let termination = service.terminate(cell_id("1")).await.unwrap();

        assert_eq!(
            termination,
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
            })
        );
    }

    #[tokio::test]
    async fn wait_to_pending_returns_after_resumed_runtime_becomes_quiescent_again() {
        let service = CodeModeService::new();

        let initial_response = service
            .execute_to_pending(ExecuteRequest {
                source: r#"
await new Promise((resolve) => setTimeout(resolve, 60_000));
text("after");
await new Promise(() => {});
"#
                .to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            initial_response,
            ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                pending_tool_call_ids: Vec::new(),
            }
        );

        let runtime_tx = service
            .inner
            .cells
            .lock()
            .await
            .get(&cell_id("1"))
            .unwrap()
            .runtime_tx
            .clone();
        runtime_tx
            .send(RuntimeCommand::TimeoutFired { id: 1 })
            .unwrap();

        let resumed_response = tokio::time::timeout(
            Duration::from_secs(1),
            service.wait_to_pending(WaitToPendingRequest {
                cell_id: cell_id("1"),
            }),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            resumed_response,
            WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "after".to_string(),
                }],
                pending_tool_call_ids: Vec::new(),
            })
        );

        let termination = service.terminate(cell_id("1")).await.unwrap();

        assert_eq!(
            termination,
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
            })
        );
    }

    #[tokio::test]
    async fn wait_to_pending_returns_completed_after_resumed_runtime_finishes() {
        let service = CodeModeService::new();

        let initial_response = service
            .execute_to_pending(ExecuteRequest {
                source: r#"
await new Promise((resolve) => setTimeout(resolve, 60_000));
text("done");
"#
                .to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            initial_response,
            ExecuteToPendingOutcome::Pending {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                pending_tool_call_ids: Vec::new(),
            }
        );

        let runtime_tx = service
            .inner
            .cells
            .lock()
            .await
            .get(&cell_id("1"))
            .unwrap()
            .runtime_tx
            .clone();
        runtime_tx
            .send(RuntimeCommand::TimeoutFired { id: 1 })
            .unwrap();

        let resumed_response = tokio::time::timeout(
            Duration::from_secs(1),
            service.wait_to_pending(WaitToPendingRequest {
                cell_id: cell_id("1"),
            }),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(
            resumed_response,
            WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Completed(
                RuntimeResponse::Result {
                    cell_id: cell_id("1"),
                    content_items: vec![FunctionCallOutputContentItem::InputText {
                        text: "done".to_string(),
                    }],
                    error_text: None,
                }
            ))
        );
    }

    #[tokio::test]
    async fn v8_console_is_not_exposed_on_global_this() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"text(String(Object.hasOwn(globalThis, "console")));"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "false".to_string(),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn date_locale_string_formats_with_icu_data() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
const value = new Date("2025-01-02T03:04:05Z")
  .toLocaleString("fr-FR", {
    weekday: "long",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    timeZone: "UTC",
  });
text(value);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn intl_date_time_format_formats_with_icu_data() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
const formatter = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  month: "long",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
  timeZone: "UTC",
});
text(formatter.format(new Date("2025-01-02T03:04:05Z")));
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn output_helpers_return_undefined() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
const returnsUndefined = [
  text("first"),
  image("data:image/png;base64,AAA"),
  notify("ping"),
].map((value) => value === undefined);
text(JSON.stringify(returnsUndefined));
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "first".to_string(),
                    },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,AAA".to_string(),
                        detail: Some(crate::DEFAULT_IMAGE_DETAIL),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "[true,true,true]".to_string(),
                    },
                ],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_accepts_raw_mcp_image_block_with_original_detail() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  type: "image",
  data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  mimeType: "image/png",
  _meta: { "codex/imageDetail": "original" },
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::Original),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn generated_image_helper_appends_image_and_output_hint() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
generatedImage({
  image_url: "data:image/png;base64,AAA",
  output_hint: "generated image save hint",
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,AAA".to_string(),
                        detail: Some(crate::DEFAULT_IMAGE_DETAIL),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "generated image save hint".to_string(),
                    },
                ],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_second_arg_overrides_explicit_object_detail() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image(
  {
    image_url: "data:image/png;base64,AAA",
    detail: "high",
  },
  "original",
);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: Some(crate::ImageDetail::Original),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_second_arg_overrides_raw_mcp_image_detail() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image(
  {
    type: "image",
    data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
    mimeType: "image/png",
    _meta: { "codex/imageDetail": "original" },
  },
  "high",
);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::High),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_accepts_low_detail() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "low",
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: Some(crate::ImageDetail::Low),
                }],
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helpers_reject_remote_urls() {
        for image_url in [
            "http://example.com/image.jpg",
            "https://example.com/image.jpg",
        ] {
            for source in [
                format!("image({image_url:?});"),
                format!("generatedImage({{ image_url: {image_url:?} }});"),
            ] {
                let service = CodeModeService::new();

                let response = execute(
                    &service,
                    ExecuteRequest {
                        source,
                        yield_time_ms: None,
                        ..execute_request("")
                    },
                )
                .await;

                assert_eq!(
                    response,
                    RuntimeResponse::Result {
                        cell_id: cell_id("1"),
                        content_items: Vec::new(),
                        error_text: Some(
                            "Tool call failed: remote image URLs are not supported in tool outputs. Pass a base64 data URI instead".to_string(),
                        ),
                    }
                );
            }
        }
    }

    #[tokio::test]
    async fn image_helper_rejects_unsupported_detail() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  image_url: "data:image/png;base64,AAA",
  detail: "medium",
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: Some(
                    "image detail must be one of: auto, low, high, original".to_string()
                ),
            }
        );
    }

    #[tokio::test]
    async fn image_helper_rejects_raw_mcp_result_container() {
        let service = CodeModeService::new();

        let response = execute(
            &service,
            ExecuteRequest {
                source: r#"
image({
  content: [
    {
      type: "image",
      data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
      mimeType: "image/png",
      _meta: { "codex/imageDetail": "original" },
    },
  ],
  isError: false,
});
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
        )
        .await;

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: Vec::new(),
                error_text: Some(
                    "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block".to_string(),
                ),
            }
        );
    }

    #[tokio::test]
    async fn wait_reports_missing_cell_separately_from_runtime_results() {
        let service = CodeModeService::new();

        let response = service
            .wait(WaitRequest {
                cell_id: cell_id("missing"),
                yield_time_ms: 1,
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            WaitOutcome::MissingCell(RuntimeResponse::Result {
                cell_id: cell_id("missing"),
                content_items: Vec::new(),
                error_text: Some("exec cell missing not found".to_string()),
            })
        );
    }

    #[tokio::test]
    async fn terminate_waits_for_runtime_shutdown_before_responding() {
        let inner = test_inner();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (initial_response_tx, initial_response_rx) = oneshot::channel();
        let (runtime_event_tx, _runtime_event_rx) = mpsc::unbounded_channel();
        let (runtime_tx, runtime_control_tx, runtime_terminate_handle) = spawn_runtime(
            HashMap::new(),
            ExecuteRequest {
                source: "await new Promise(() => {})".to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
            runtime_event_tx,
            PendingRuntimeMode::Continue,
        )
        .unwrap();

        tokio::spawn(run_cell_control(
            inner,
            CellControlContext {
                cell_id: cell_id("cell-1"),
                runtime_tx: runtime_tx.clone(),
                runtime_control_tx,
                pending_mode: PendingRuntimeMode::Continue,
                runtime_terminate_handle,
                cancellation_token: tokio_util::sync::CancellationToken::new(),
            },
            event_rx,
            control_rx,
            CellResponseSender::Runtime(initial_response_tx),
            Some(/*initial_yield_time_ms*/ 60_000),
        ));

        event_tx.send(RuntimeEvent::Started).unwrap();
        event_tx.send(RuntimeEvent::YieldRequested).unwrap();
        assert_eq!(
            initial_response_rx.await.unwrap(),
            Ok(RuntimeResponse::Yielded {
                cell_id: cell_id("cell-1"),
                content_items: Vec::new(),
            })
        );

        let (terminate_response_tx, terminate_response_rx) = oneshot::channel();
        control_tx
            .send(CellControlCommand::Terminate {
                response_tx: terminate_response_tx,
            })
            .unwrap();
        let terminate_response = async { terminate_response_rx.await.unwrap() };
        tokio::pin!(terminate_response);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), terminate_response.as_mut())
                .await
                .is_err()
        );

        drop(event_tx);

        assert_eq!(
            terminate_response.await,
            Ok(RuntimeResponse::Terminated {
                cell_id: cell_id("cell-1"),
                content_items: Vec::new(),
            })
        );

        let _ = runtime_tx.send(RuntimeCommand::Terminate);
    }
}

#[cfg(test)]
#[path = "service_contract_tests.rs"]
mod contract_tests;
