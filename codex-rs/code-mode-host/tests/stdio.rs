#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSession;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ExecuteRequest;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ProcessOwnedCodeModeSessionProvider;
use codex_code_mode::RuntimeResponse;
use codex_code_mode::ToolDefinition;
use codex_code_mode::ToolInvocationFuture;
use codex_code_mode::WaitOutcome;
use codex_code_mode::WaitRequest;
use codex_code_mode::host::MAX_FRAME_BYTES;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RecordingDelegate {
    invocations: Mutex<Vec<CodeModeNestedToolCall>>,
    notifications: Mutex<Vec<(String, CellId, String)>>,
    closed_cells: Mutex<Vec<CellId>>,
}

#[derive(Debug, Eq, PartialEq)]
enum CallbackEvent {
    Started(String),
    Cancelled(String),
    CellClosed(CellId),
}

struct CancellationDelegate {
    events_tx: mpsc::UnboundedSender<CallbackEvent>,
    fast_tool_release: Semaphore,
    slow_tool_started: Semaphore,
    hold_slow_cleanup: AtomicBool,
    slow_cleanup_release: Semaphore,
}

struct OversizedResultDelegate;

impl CodeModeSessionDelegate for OversizedResultDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async { Ok(json!("x".repeat(MAX_FRAME_BYTES))) })
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

impl CancellationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<CallbackEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
                fast_tool_release: Semaphore::new(/*permits*/ 0),
                slow_tool_started: Semaphore::new(/*permits*/ 0),
                hold_slow_cleanup: AtomicBool::new(false),
                slow_cleanup_release: Semaphore::new(/*permits*/ 0),
            }),
            events_rx,
        )
    }

    #[cfg(unix)]
    fn hold_slow_cleanup(&self) {
        self.hold_slow_cleanup.store(true, Ordering::Release);
    }

    #[cfg(unix)]
    fn release_slow_cleanup(&self) {
        self.slow_cleanup_release.add_permits(1);
    }
}

impl CodeModeSessionDelegate for CancellationDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let tool_name = invocation.tool_name.name.clone();
            if tool_name == "tool_call_barrier" {
                let permit = self
                    .slow_tool_started
                    .acquire()
                    .await
                    .map_err(|_| "slow tool barrier closed".to_string())?;
                permit.forget();
                return Ok(json!({ "tool": tool_name }));
            }
            let _ = self
                .events_tx
                .send(CallbackEvent::Started(tool_name.clone()));
            if tool_name == "tool_call_slow" {
                self.slow_tool_started.add_permits(1);
                cancellation_token.cancelled().await;
                let _ = self.events_tx.send(CallbackEvent::Cancelled(tool_name));
                if self.hold_slow_cleanup.load(Ordering::Acquire) {
                    let permit = self
                        .slow_cleanup_release
                        .acquire()
                        .await
                        .map_err(|_| "slow tool cleanup release closed".to_string())?;
                    permit.forget();
                }
                return Err("slow tool cancelled".to_string());
            }
            let permit = self
                .fast_tool_release
                .acquire()
                .await
                .map_err(|_| "fast tool release closed".to_string())?;
            permit.forget();
            Ok(json!({ "tool": tool_name }))
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

    fn cell_closed(&self, cell_id: &CellId) {
        let _ = self
            .events_tx
            .send(CallbackEvent::CellClosed(cell_id.clone()));
    }
}

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.invocations
            .lock()
            .expect("invocations lock")
            .push(invocation);
        Box::pin(async { Ok(json!({ "value": "output" })) })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.notifications
            .lock()
            .expect("notifications lock")
            .push((call_id, cell_id, text));
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.closed_cells
            .lock()
            .expect("closed cells lock")
            .push(cell_id.clone());
    }
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: None,
        max_output_tokens: None,
    }
}

async fn execute(session: &Arc<dyn CodeModeSession>, request: ExecuteRequest) -> RuntimeResponse {
    session
        .execute(request)
        .await
        .expect("start execution")
        .initial_response()
        .await
        .expect("initial response")
}

async fn execute_to_terminal(
    session: &Arc<dyn CodeModeSession>,
    request: ExecuteRequest,
) -> RuntimeResponse {
    let started = session.execute(request).await.expect("start execution");
    let mut response = started.initial_response().await.expect("initial response");
    loop {
        match response {
            RuntimeResponse::Yielded { cell_id, .. } => {
                response = match session
                    .wait(WaitRequest {
                        cell_id,
                        yield_time_ms: 60_000,
                    })
                    .await
                    .expect("wait for terminal response")
                {
                    WaitOutcome::LiveCell(response) | WaitOutcome::MissingCell(response) => {
                        response
                    }
                };
            }
            response => return response,
        }
    }
}

async fn next_callback_event(
    events_rx: &mut mpsc::UnboundedReceiver<CallbackEvent>,
) -> CallbackEvent {
    tokio::time::timeout(Duration::from_secs(5), events_rx.recv())
        .await
        .expect("callback event timeout")
        .expect("callback event stream closed")
}

#[tokio::test]
async fn remote_session_persists_values_forwards_delegates_and_controls_cells() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let delegate = Arc::new(RecordingDelegate::default());
    let session = provider
        .create_session(delegate.clone())
        .await
        .expect("create remote session");

    assert_eq!(
        execute(&session, execute_request(r#"store("key", "persisted");"#),).await,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );

    let mut callback_request = execute_request(
        r#"
const result = await tools.echo({ value: String(load("key")) });
notify("notice");
text(result.value);
"#,
    );
    callback_request.tool_call_id = "call-2".to_string();
    callback_request.enabled_tools = vec![ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName::plain("echo"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }];
    assert_eq!(
        execute(&session, callback_request).await,
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "output".to_string(),
            }],
            error_text: None,
        }
    );
    assert_eq!(
        *delegate.invocations.lock().expect("invocations lock"),
        vec![CodeModeNestedToolCall {
            cell_id: cell_id("2"),
            runtime_tool_call_id: "tool-1".to_string(),
            tool_name: ToolName::plain("echo"),
            tool_kind: CodeModeToolKind::Function,
            input: Some(json!({ "value": "persisted" })),
        }]
    );
    assert_eq!(
        *delegate.notifications.lock().expect("notifications lock"),
        vec![("call-2".to_string(), cell_id("2"), "notice".to_string())]
    );

    let mut pending_request = execute_request("await new Promise(() => {});");
    pending_request.tool_call_id = "call-3".to_string();
    pending_request.yield_time_ms = Some(1);
    assert_eq!(
        execute(&session, pending_request).await,
        RuntimeResponse::Yielded {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        session
            .wait(WaitRequest {
                cell_id: cell_id("3"),
                yield_time_ms: 1,
            })
            .await
            .expect("wait for cell"),
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        session
            .terminate(cell_id("3"))
            .await
            .expect("terminate cell"),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("3"),
            content_items: Vec::new(),
        })
    );

    session.shutdown().await.expect("shutdown remote session");
    assert_eq!(
        *delegate.closed_cells.lock().expect("closed cells lock"),
        vec![cell_id("1"), cell_id("2"), cell_id("3")]
    );
}

#[tokio::test]
async fn dropping_long_wait_releases_observer_before_next_wait() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");
    let mut request = execute_request("await new Promise(() => {});");
    request.yield_time_ms = Some(1);
    let started = session.execute(request).await.expect("start execution");
    let running_cell_id = started.cell_id.clone();
    assert_eq!(
        started.initial_response().await.expect("initial response"),
        RuntimeResponse::Yielded {
            cell_id: running_cell_id.clone(),
            content_items: Vec::new(),
        }
    );

    let wait_session = Arc::clone(&session);
    let wait_cell_id = running_cell_id.clone();
    let first_wait = tokio::spawn(async move {
        wait_session
            .wait(WaitRequest {
                cell_id: wait_cell_id,
                yield_time_ms: 60_000,
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    first_wait.abort();
    let _ = first_wait.await;

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            session.wait(WaitRequest {
                cell_id: running_cell_id.clone(),
                yield_time_ms: 1,
            })
        )
        .await
        .expect("second wait timeout")
        .expect("second wait"),
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: running_cell_id.clone(),
            content_items: Vec::new(),
        })
    );
    session
        .terminate(running_cell_id)
        .await
        .expect("terminate cell");
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn unawaited_slow_tool_is_cancelled_after_parallel_tools_complete() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let (delegate, mut events_rx) = CancellationDelegate::new();
    let session = provider
        .create_session(delegate.clone())
        .await
        .expect("create remote session");
    let mut request = execute_request(
        r#"
await (async () => {
text("hello world");
yield_control();
await Promise.all([
    tools.tool_call_a({}),
    tools.tool_call_b({}),
]);
text("hello");
tools.tool_call_slow({});
await tools.tool_call_barrier({});
return;
})();
"#,
    );
    request.enabled_tools = [
        "tool_call_a",
        "tool_call_b",
        "tool_call_slow",
        "tool_call_barrier",
    ]
    .into_iter()
    .map(|name| ToolDefinition {
        name: name.to_string(),
        tool_name: ToolName::plain(name),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    })
    .collect();

    let started = session.execute(request).await.expect("start execution");
    let running_cell_id = started.cell_id.clone();
    assert_eq!(
        started.initial_response().await.expect("initial response"),
        RuntimeResponse::Yielded {
            cell_id: running_cell_id.clone(),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "hello world".to_string(),
            }],
        }
    );

    let wait_session = Arc::clone(&session);
    let wait_cell_id = running_cell_id.clone();
    let wait_task = tokio::spawn(async move {
        wait_session
            .wait(WaitRequest {
                cell_id: wait_cell_id,
                yield_time_ms: 60_000,
            })
            .await
    });

    let mut parallel_tools = vec![
        next_callback_event(&mut events_rx).await,
        next_callback_event(&mut events_rx).await,
    ];
    parallel_tools.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    assert_eq!(
        parallel_tools,
        vec![
            CallbackEvent::Started("tool_call_a".to_string()),
            CallbackEvent::Started("tool_call_b".to_string()),
        ]
    );
    delegate.fast_tool_release.add_permits(2);

    assert_eq!(
        next_callback_event(&mut events_rx).await,
        CallbackEvent::Started("tool_call_slow".to_string())
    );
    let mut closure_events = vec![
        next_callback_event(&mut events_rx).await,
        next_callback_event(&mut events_rx).await,
    ];
    closure_events.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    assert_eq!(
        closure_events,
        vec![
            CallbackEvent::Cancelled("tool_call_slow".to_string()),
            CallbackEvent::CellClosed(running_cell_id.clone()),
        ]
    );
    assert_eq!(
        wait_task
            .await
            .expect("wait task")
            .expect("wait for terminal response"),
        WaitOutcome::LiveCell(RuntimeResponse::Result {
            cell_id: running_cell_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "hello".to_string(),
            }],
            error_text: None,
        })
    );
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn oversized_execute_request_does_not_close_the_shared_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");
    let error = session
        .execute(execute_request(&"x".repeat(MAX_FRAME_BYTES)))
        .await
        .err()
        .expect("oversized execute should fail");
    assert!(
        error.contains("IPC frame limit"),
        "unexpected error: {error}"
    );

    assert_eq!(
        execute(&session, execute_request(r#"text("still alive");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "still alive".to_string(),
            }],
            error_text: None,
        }
    );
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn oversized_delegate_payloads_fail_only_the_tool_call() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(OversizedResultDelegate))
        .await
        .expect("create remote session");
    let tool = |name: &str| ToolDefinition {
        name: name.to_string(),
        tool_name: ToolName::plain(name),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    };

    let mut oversized_argument = execute_request(&format!(
        r#"
try {{
    await tools.big_argument({{ value: "x".repeat({MAX_FRAME_BYTES}) }});
}} catch (_) {{
    text("argument rejected");
}}
"#
    ));
    oversized_argument.enabled_tools = vec![tool("big_argument")];
    oversized_argument.yield_time_ms = Some(60_000);
    assert_eq!(
        execute_to_terminal(&session, oversized_argument).await,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "argument rejected".to_string(),
            }],
            error_text: None,
        }
    );

    let mut oversized_result = execute_request(
        r#"
try {
    await tools.big_result({});
} catch (_) {
    text("result rejected");
}
"#,
    );
    oversized_result.enabled_tools = vec![tool("big_result")];
    oversized_result.yield_time_ms = Some(60_000);
    assert_eq!(
        execute_to_terminal(&session, oversized_result).await,
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "result rejected".to_string(),
            }],
            error_text: None,
        }
    );

    assert_eq!(
        execute(&session, execute_request(r#"text("still alive");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("3"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "still alive".to_string(),
            }],
            error_text: None,
        }
    );
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn oversized_initial_response_does_not_close_the_shared_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");
    let started = session
        .execute(execute_request(&format!(
            r#"text("x".repeat({MAX_FRAME_BYTES}));"#
        )))
        .await
        .expect("start oversized response");
    let error = started
        .initial_response()
        .await
        .expect_err("oversized initial response should fail");
    assert!(
        error.contains("IPC frame limit"),
        "unexpected error: {error}"
    );

    assert_eq!(
        execute(&session, execute_request(r#"text("still alive");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "still alive".to_string(),
            }],
            error_text: None,
        }
    );
    session.shutdown().await.expect("shutdown remote session");
}

#[cfg(unix)]
#[tokio::test]
async fn child_process_loss_cleans_up_and_rebuilds_the_shared_host() {
    let host_program =
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary");
    let proxy_dir = tempfile::tempdir().expect("create host proxy directory");
    let proxy_program = proxy_dir.path().join("host-proxy.sh");
    let pid_path = proxy_dir.path().join("host.pid");
    std::fs::write(
        &proxy_program,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nexec '{}'\n",
            pid_path.display(),
            host_program.display()
        ),
    )
    .expect("write host proxy");
    let mut permissions = std::fs::metadata(&proxy_program)
        .expect("host proxy metadata")
        .permissions();
    permissions.set_mode(/*mode*/ 0o700);
    std::fs::set_permissions(&proxy_program, permissions).expect("make host proxy executable");

    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(proxy_program);
    let (delegate_a, mut events_a) = CancellationDelegate::new();
    delegate_a.hold_slow_cleanup();
    let delegate_b = Arc::new(RecordingDelegate::default());
    let session_a = provider
        .create_session(delegate_a.clone())
        .await
        .expect("create first remote session");
    let session_b = provider
        .create_session(delegate_b.clone())
        .await
        .expect("create second remote session");

    let mut request_a = execute_request("await tools.tool_call_slow({});");
    request_a.yield_time_ms = Some(1);
    request_a.enabled_tools = vec![ToolDefinition {
        name: "tool_call_slow".to_string(),
        tool_name: ToolName::plain("tool_call_slow"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }];
    let started_a = session_a
        .execute(request_a)
        .await
        .expect("start first cell");
    let cell_a = started_a.cell_id.clone();
    assert_eq!(
        started_a
            .initial_response()
            .await
            .expect("first initial response"),
        RuntimeResponse::Yielded {
            cell_id: cell_a.clone(),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        next_callback_event(&mut events_a).await,
        CallbackEvent::Started("tool_call_slow".to_string())
    );

    let mut request_b = execute_request("await new Promise(() => {});");
    request_b.yield_time_ms = Some(1);
    let started_b = session_b
        .execute(request_b)
        .await
        .expect("start second cell");
    let cell_b = started_b.cell_id.clone();
    assert_eq!(
        started_b
            .initial_response()
            .await
            .expect("second initial response"),
        RuntimeResponse::Yielded {
            cell_id: cell_b.clone(),
            content_items: Vec::new(),
        }
    );

    let wait_a_session = Arc::clone(&session_a);
    let wait_a_cell = cell_a.clone();
    let wait_a = tokio::spawn(async move {
        wait_a_session
            .wait(WaitRequest {
                cell_id: wait_a_cell,
                yield_time_ms: 60_000,
            })
            .await
    });
    let wait_b_session = Arc::clone(&session_b);
    let wait_b_cell = cell_b.clone();
    let wait_b = tokio::spawn(async move {
        wait_b_session
            .wait(WaitRequest {
                cell_id: wait_b_cell,
                yield_time_ms: 60_000,
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(pid) = std::fs::read_to_string(&pid_path)
                && let Ok(pid) = pid.trim().parse::<u32>()
            {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host pid timeout");
    let kill_status = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .expect("kill host process");
    assert!(kill_status.success());

    assert!(
        tokio::time::timeout(Duration::from_secs(5), wait_a)
            .await
            .expect("first wait failure timeout")
            .expect("first wait task")
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), wait_b)
            .await
            .expect("second wait failure timeout")
            .expect("second wait task")
            .is_err()
    );
    let closure_events = [
        next_callback_event(&mut events_a).await,
        next_callback_event(&mut events_a).await,
    ];
    assert!(closure_events.contains(&CallbackEvent::Cancelled("tool_call_slow".to_string())));
    assert!(closure_events.contains(&CallbackEvent::CellClosed(cell_a.clone())));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if delegate_b
                .closed_cells
                .lock()
                .expect("closed cells lock")
                .contains(&cell_b)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unrelated session cleanup timeout");

    assert_eq!(
        execute(&session_b, execute_request(r#"text("replacement");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("g2:1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "replacement".to_string(),
            }],
            error_text: None,
        }
    );
    let stale_error = session_b
        .wait(WaitRequest {
            cell_id: cell_b.clone(),
            yield_time_ms: 1,
        })
        .await
        .expect_err("stale cell should be rejected");
    assert!(stale_error.contains("stale code-mode host generation"));

    tokio::time::timeout(Duration::from_secs(5), session_a.shutdown())
        .await
        .expect("failed session shutdown timeout")
        .expect("shutdown failed session");
    tokio::time::timeout(Duration::from_secs(5), session_b.shutdown())
        .await
        .expect("unrelated session shutdown timeout")
        .expect("shutdown replacement session");

    delegate_a.release_slow_cleanup();
    tokio::task::yield_now().await;
    assert!(matches!(
        events_a.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}
