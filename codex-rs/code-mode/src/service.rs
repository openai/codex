use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use async_trait::async_trait;
use codex_protocol::ToolName;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::FunctionCallOutputContentItem;
use crate::runtime::DEFAULT_EXEC_YIELD_TIME_MS;
use crate::runtime::ExecuteRequest;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::RuntimeResponse;
use crate::runtime::TurnMessage;
use crate::runtime::WaitRequest;
use crate::runtime::spawn_runtime;

mod execute_until_done;

pub use execute_until_done::ExecuteUntilDoneRequest;

#[async_trait]
pub trait CodeModeTurnHost: Send + Sync {
    async fn invoke_tool(
        &self,
        tool_name: ToolName,
        input: Option<JsonValue>,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String>;

    async fn notify(&self, call_id: String, cell_id: String, text: String) -> Result<(), String>;
}

#[derive(Clone)]
struct SessionHandle {
    control_tx: mpsc::UnboundedSender<SessionControlCommand>,
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
}

struct Inner {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    sessions: Mutex<HashMap<String, SessionHandle>>,
    turn_message_tx: async_channel::Sender<TurnMessage>,
    turn_message_rx: async_channel::Receiver<TurnMessage>,
    next_cell_id: AtomicU64,
}

pub struct CodeModeService {
    inner: Arc<Inner>,
}

struct StartedExecution {
    control_tx: mpsc::UnboundedSender<SessionControlCommand>,
    response_rx: oneshot::Receiver<RuntimeResponse>,
}

impl CodeModeService {
    pub fn new() -> Self {
        let (turn_message_tx, turn_message_rx) = async_channel::unbounded();

        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                sessions: Mutex::new(HashMap::new()),
                turn_message_tx,
                turn_message_rx,
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    pub async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.inner.stored_values.lock().await.clone()
    }

    pub async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        *self.inner.stored_values.lock().await = values;
    }

    async fn start_execution(&self, request: ExecuteRequest) -> Result<StartedExecution, String> {
        let mut sessions = self.inner.sessions.lock().await;
        let cell_id = self
            .inner
            .next_cell_id
            .fetch_add(1, Ordering::Relaxed)
            .to_string();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (runtime_tx, runtime_terminate_handle) = spawn_runtime(request.clone(), event_tx)?;
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = oneshot::channel();

        sessions.insert(
            cell_id.clone(),
            SessionHandle {
                control_tx: control_tx.clone(),
                runtime_tx: runtime_tx.clone(),
            },
        );
        drop(sessions);

        tokio::spawn(run_session_control(
            Arc::clone(&self.inner),
            SessionControlContext {
                cell_id,
                runtime_tx,
                runtime_terminate_handle,
            },
            event_rx,
            control_rx,
            response_tx,
            request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS),
        ));

        Ok(StartedExecution {
            control_tx,
            response_rx,
        })
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<RuntimeResponse, String> {
        self.start_execution(request)
            .await?
            .response_rx
            .await
            .map_err(|_| "exec runtime ended unexpectedly".to_string())
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<RuntimeResponse, String> {
        let cell_id = request.cell_id.clone();
        let handle = self
            .inner
            .sessions
            .lock()
            .await
            .get(&request.cell_id)
            .cloned();
        let Some(handle) = handle else {
            return Ok(missing_cell_response(cell_id));
        };
        let (response_tx, response_rx) = oneshot::channel();
        let control_message = if request.terminate {
            SessionControlCommand::Terminate { response_tx }
        } else {
            SessionControlCommand::Poll {
                yield_time_ms: request.yield_time_ms,
                response_tx,
            }
        };
        if handle.control_tx.send(control_message).is_err() {
            return Ok(missing_cell_response(cell_id));
        }
        match response_rx.await {
            Ok(response) => Ok(response),
            Err(_) => Ok(missing_cell_response(request.cell_id)),
        }
    }

    pub fn start_turn_worker(&self, host: Arc<dyn CodeModeTurnHost>) -> CodeModeTurnWorker {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let inner = Arc::clone(&self.inner);
        let turn_message_rx = self.inner.turn_message_rx.clone();

        tokio::spawn(async move {
            loop {
                let next_message = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    message = turn_message_rx.recv() => message.ok(),
                };
                let Some(next_message) = next_message else {
                    break;
                };
                match next_message {
                    TurnMessage::Notify {
                        cell_id,
                        call_id,
                        text,
                    } => {
                        if let Err(err) = host.notify(call_id, cell_id.clone(), text).await {
                            warn!(
                                "failed to deliver code mode notification for cell {cell_id}: {err}"
                            );
                        }
                    }
                    TurnMessage::ToolCall {
                        cell_id,
                        id,
                        name,
                        input,
                    } => {
                        let host = Arc::clone(&host);
                        let inner = Arc::clone(&inner);
                        tokio::spawn(async move {
                            let response = host
                                .invoke_tool(name, input, CancellationToken::new())
                                .await;
                            let runtime_tx = inner
                                .sessions
                                .lock()
                                .await
                                .get(&cell_id)
                                .map(|handle| handle.runtime_tx.clone());
                            let Some(runtime_tx) = runtime_tx else {
                                return;
                            };
                            let command = match response {
                                Ok(result) => RuntimeCommand::ToolResponse { id, result },
                                Err(error_text) => RuntimeCommand::ToolError { id, error_text },
                            };
                            let _ = runtime_tx.send(command);
                        });
                    }
                }
            }
        });

        CodeModeTurnWorker {
            shutdown_tx: Some(shutdown_tx),
        }
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CodeModeTurnWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl Drop for CodeModeTurnWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

enum SessionControlCommand {
    Poll {
        yield_time_ms: u64,
        response_tx: oneshot::Sender<RuntimeResponse>,
    },
    Terminate {
        response_tx: oneshot::Sender<RuntimeResponse>,
    },
}

struct PendingResult {
    content_items: Vec<FunctionCallOutputContentItem>,
    stored_values: HashMap<String, JsonValue>,
    error_text: Option<String>,
}

struct SessionControlContext {
    cell_id: String,
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    runtime_terminate_handle: v8::IsolateHandle,
}

fn missing_cell_response(cell_id: String) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
        stored_values: HashMap::new(),
    }
}

fn pending_result_response(cell_id: &str, result: PendingResult) -> RuntimeResponse {
    RuntimeResponse::Result {
        cell_id: cell_id.to_string(),
        content_items: result.content_items,
        stored_values: result.stored_values,
        error_text: result.error_text,
    }
}

fn send_or_buffer_result(
    cell_id: &str,
    result: PendingResult,
    response_tx: &mut Option<oneshot::Sender<RuntimeResponse>>,
    pending_result: &mut Option<PendingResult>,
) -> bool {
    if let Some(response_tx) = response_tx.take() {
        let _ = response_tx.send(pending_result_response(cell_id, result));
        return true;
    }

    *pending_result = Some(result);
    false
}

async fn run_session_control(
    inner: Arc<Inner>,
    context: SessionControlContext,
    mut event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    mut control_rx: mpsc::UnboundedReceiver<SessionControlCommand>,
    initial_response_tx: oneshot::Sender<RuntimeResponse>,
    initial_yield_time_ms: u64,
) {
    let SessionControlContext {
        cell_id,
        runtime_tx,
        runtime_terminate_handle,
    } = context;
    let mut content_items = Vec::new();
    let mut pending_result: Option<PendingResult> = None;
    let mut response_tx = Some(initial_response_tx);
    let mut termination_requested = false;
    let mut runtime_closed = false;
    let mut yield_timer: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        tokio::select! {
            maybe_event = async {
                if runtime_closed {
                    std::future::pending::<Option<RuntimeEvent>>().await
                } else {
                    event_rx.recv().await
                }
            } => {
                let Some(event) = maybe_event else {
                    runtime_closed = true;
                    if termination_requested {
                        if let Some(response_tx) = response_tx.take() {
                            let _ = response_tx.send(RuntimeResponse::Terminated {
                                cell_id: cell_id.clone(),
                                content_items: std::mem::take(&mut content_items),
                            });
                        }
                        break;
                    }
                    if pending_result.is_none() {
                        let result = PendingResult {
                            content_items: std::mem::take(&mut content_items),
                            stored_values: HashMap::new(),
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
                        yield_timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(initial_yield_time_ms))));
                    }
                    RuntimeEvent::ContentItem(item) => {
                        content_items.push(item);
                    }
                    RuntimeEvent::YieldRequested => {
                        yield_timer = None;
                        if let Some(response_tx) = response_tx.take() {
                            let _ = response_tx.send(RuntimeResponse::Yielded {
                                cell_id: cell_id.clone(),
                                content_items: std::mem::take(&mut content_items),
                            });
                        }
                    }
                    RuntimeEvent::Notify { call_id, text } => {
                        let _ = inner.turn_message_tx.send(TurnMessage::Notify {
                            cell_id: cell_id.clone(),
                            call_id,
                            text,
                        }).await;
                    }
                    RuntimeEvent::ToolCall { id, name, input } => {
                        let _ = inner.turn_message_tx.send(TurnMessage::ToolCall {
                            cell_id: cell_id.clone(),
                            id,
                            name,
                            input,
                        }).await;
                    }
                    RuntimeEvent::Result {
                        stored_values,
                        error_text,
                    } => {
                        yield_timer = None;
                        if termination_requested {
                            if let Some(response_tx) = response_tx.take() {
                                let _ = response_tx.send(RuntimeResponse::Terminated {
                                    cell_id: cell_id.clone(),
                                    content_items: std::mem::take(&mut content_items),
                                });
                            }
                            break;
                        }
                        let result = PendingResult {
                            content_items: std::mem::take(&mut content_items),
                            stored_values,
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
            maybe_command = control_rx.recv() => {
                let Some(command) = maybe_command else {
                    break;
                };
                match command {
                    SessionControlCommand::Poll {
                        yield_time_ms,
                        response_tx: next_response_tx,
                    } => {
                        if let Some(result) = pending_result.take() {
                            let _ = next_response_tx.send(pending_result_response(&cell_id, result));
                            break;
                        }
                        response_tx = Some(next_response_tx);
                        yield_timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(yield_time_ms))));
                    }
                    SessionControlCommand::Terminate { response_tx: next_response_tx } => {
                        if let Some(result) = pending_result.take() {
                            let _ = next_response_tx.send(pending_result_response(&cell_id, result));
                            break;
                        }

                        response_tx = Some(next_response_tx);
                        termination_requested = true;
                        yield_timer = None;
                        let _ = runtime_tx.send(RuntimeCommand::Terminate);
                        let _ = runtime_terminate_handle.terminate_execution();
                        if runtime_closed {
                            if let Some(response_tx) = response_tx.take() {
                                let _ = response_tx.send(RuntimeResponse::Terminated {
                                    cell_id: cell_id.clone(),
                                    content_items: std::mem::take(&mut content_items),
                                });
                            }
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
                if let Some(response_tx) = response_tx.take() {
                    let _ = response_tx.send(RuntimeResponse::Yielded {
                        cell_id: cell_id.clone(),
                        content_items: std::mem::take(&mut content_items),
                    });
                }
            }
        }
    }

    let _ = runtime_tx.send(RuntimeCommand::Terminate);
    inner.sessions.lock().await.remove(&cell_id);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    use super::CodeModeService;
    use super::ExecuteUntilDoneRequest;
    use super::Inner;
    use super::RuntimeCommand;
    use super::RuntimeResponse;
    use super::SessionControlCommand;
    use super::SessionControlContext;
    use super::run_session_control;
    use crate::FunctionCallOutputContentItem;
    use crate::runtime::ExecuteRequest;
    use crate::runtime::RuntimeEvent;
    use crate::runtime::spawn_runtime;

    fn execute_request(source: &str) -> ExecuteRequest {
        ExecuteRequest {
            tool_call_id: "call_1".to_string(),
            enabled_tools: Vec::new(),
            source: source.to_string(),
            stored_values: HashMap::new(),
            yield_time_ms: Some(1),
            max_output_tokens: None,
        }
    }

    fn test_inner() -> Arc<Inner> {
        let (turn_message_tx, turn_message_rx) = async_channel::unbounded();
        Arc::new(Inner {
            stored_values: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            turn_message_tx,
            turn_message_rx,
            next_cell_id: AtomicU64::new(1),
        })
    }

    #[tokio::test]
    async fn synchronous_exit_returns_successfully() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
                source: r#"text("before"); exit(); text("after");"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn v8_console_is_not_exposed_on_global_this() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
                source: r#"text(String(Object.hasOwn(globalThis, "console")));"#.to_string(),
                yield_time_ms: None,
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "false".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn date_locale_string_formats_with_icu_data() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
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
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn intl_date_time_format_formats_with_icu_data() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
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
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "jeudi 2 janvier \u{e0} 03:04:05".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn output_helpers_return_undefined() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
                source: r#"
const returnsUndefined = [
  text("first"),
  image("https://example.com/image.jpg"),
  notify("ping"),
].map((value) => value === undefined);
text(JSON.stringify(returnsUndefined));
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "first".to_string(),
                    },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "https://example.com/image.jpg".to_string(),
                        detail: Some(crate::DEFAULT_IMAGE_DETAIL),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "[true,true,true]".to_string(),
                    },
                ],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_accepts_raw_mcp_image_block_with_original_detail() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
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
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::Original),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_second_arg_overrides_explicit_object_detail() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
                source: r#"
image(
  {
    image_url: "https://example.com/image.jpg",
    detail: "low",
  },
  "original",
);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.com/image.jpg".to_string(),
                    detail: Some(crate::ImageDetail::Original),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_second_arg_overrides_raw_mcp_image_detail() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
                source: r#"
image(
  {
    type: "image",
    data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
    mimeType: "image/png",
    _meta: { "codex/imageDetail": "original" },
  },
  "low",
);
"#
                .to_string(),
                yield_time_ms: None,
                ..execute_request("")
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                    detail: Some(crate::ImageDetail::Low),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn image_helper_rejects_raw_mcp_result_container() {
        let service = CodeModeService::new();

        let response = service
            .execute(ExecuteRequest {
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
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: Vec::new(),
                stored_values: HashMap::new(),
                error_text: Some(
                    "image expects a non-empty image URL string, an object with image_url and optional detail, or a raw MCP image block".to_string(),
                ),
            }
        );
    }

    #[tokio::test]
    async fn execute_until_done_returns_quick_result() {
        let service = CodeModeService::new();

        let response = service
            .execute_until_done(ExecuteUntilDoneRequest {
                execute: ExecuteRequest {
                    source: r#"text("done");"#.to_string(),
                    yield_time_ms: Some(60_000),
                    ..execute_request("")
                },
                poll_yield_time_ms: 1,
                terminate_on_drop: true,
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn execute_until_done_drains_timer_yields() {
        let service = CodeModeService::new();

        let response = service
            .execute_until_done(ExecuteUntilDoneRequest {
                execute: ExecuteRequest {
                    source: r#"
await new Promise(resolve => setTimeout(resolve, 20));
text("done");
"#
                    .to_string(),
                    yield_time_ms: Some(1),
                    ..execute_request("")
                },
                poll_yield_time_ms: 1_000,
                terminate_on_drop: true,
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn execute_until_done_drains_explicit_yield_control() {
        let service = CodeModeService::new();

        let response = service
            .execute_until_done(ExecuteUntilDoneRequest {
                execute: ExecuteRequest {
                    source: r#"text("before"); yield_control(); text("after");"#.to_string(),
                    yield_time_ms: Some(60_000),
                    ..execute_request("")
                },
                poll_yield_time_ms: 60_000,
                terminate_on_drop: true,
            })
            .await
            .unwrap();

        assert_eq!(
            response,
            RuntimeResponse::Result {
                cell_id: "1".to_string(),
                content_items: vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "before".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "after".to_string(),
                    },
                ],
                stored_values: HashMap::new(),
                error_text: None,
            }
        );
    }

    #[tokio::test]
    async fn execute_until_done_terminates_running_cell_when_cancelled() {
        let service = Arc::new(CodeModeService::new());
        let run_service = Arc::clone(&service);
        let handle = tokio::spawn(async move {
            run_service
                .execute_until_done(ExecuteUntilDoneRequest {
                    execute: ExecuteRequest {
                        source: "await new Promise(() => {})".to_string(),
                        yield_time_ms: Some(60_000),
                        ..execute_request("")
                    },
                    poll_yield_time_ms: 60_000,
                    terminate_on_drop: true,
                })
                .await
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if service.inner.sessions.lock().await.len() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        handle.abort();
        let _ = handle.await;

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if service.inner.sessions.lock().await.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn terminate_waits_for_runtime_shutdown_before_responding() {
        let inner = test_inner();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (initial_response_tx, initial_response_rx) = oneshot::channel();
        let (runtime_event_tx, _runtime_event_rx) = mpsc::unbounded_channel();
        let (runtime_tx, runtime_terminate_handle) = spawn_runtime(
            ExecuteRequest {
                source: "await new Promise(() => {})".to_string(),
                yield_time_ms: None,
                ..execute_request("")
            },
            runtime_event_tx,
        )
        .unwrap();

        tokio::spawn(run_session_control(
            inner,
            SessionControlContext {
                cell_id: "cell-1".to_string(),
                runtime_tx: runtime_tx.clone(),
                runtime_terminate_handle,
            },
            event_rx,
            control_rx,
            initial_response_tx,
            /*initial_yield_time_ms*/ 60_000,
        ));

        event_tx.send(RuntimeEvent::Started).unwrap();
        event_tx.send(RuntimeEvent::YieldRequested).unwrap();
        assert_eq!(
            initial_response_rx.await.unwrap(),
            RuntimeResponse::Yielded {
                cell_id: "cell-1".to_string(),
                content_items: Vec::new(),
            }
        );

        let (terminate_response_tx, terminate_response_rx) = oneshot::channel();
        control_tx
            .send(SessionControlCommand::Terminate {
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
            RuntimeResponse::Terminated {
                cell_id: "cell-1".to_string(),
                content_items: Vec::new(),
            }
        );

        let _ = runtime_tx.send(RuntimeCommand::Terminate);
    }
}
