#![allow(clippy::expect_used)]

use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
use codex_code_mode::host::CapabilitySet;
use codex_code_mode::host::ClientHello;
use codex_code_mode::host::ClientToHost;
use codex_code_mode::host::DelegateRequest;
use codex_code_mode::host::DelegateResponse;
use codex_code_mode::host::FramedReader;
use codex_code_mode::host::FramedWriter;
use codex_code_mode::host::HostHello;
use codex_code_mode::host::HostRequest;
use codex_code_mode::host::HostResponse;
use codex_code_mode::host::HostToClient;
use codex_code_mode::host::MAX_FRAME_BYTES;
use codex_code_mode::host::ProtocolVersion;
use codex_code_mode::host::RequestId;
use codex_code_mode::host::SessionId;
use codex_code_mode::host::SupportedProtocolVersions;
use codex_code_mode::host::WireResult;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RecordingDelegate {
    invocations: Mutex<Vec<CodeModeNestedToolCall>>,
    notifications: Mutex<Vec<(String, CellId, String)>>,
    closed_cells: Mutex<Vec<CellId>>,
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

fn request_id(value: i64) -> RequestId {
    RequestId::new(value)
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
async fn oversized_output_errors_the_cell_without_wedging_the_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");

    let accepted_output_bytes = 9 * 1024 * 1024;
    let accepted = execute(
        &session,
        execute_request(&format!(r#"text("x".repeat({accepted_output_bytes}));"#)),
    )
    .await;
    let RuntimeResponse::Result {
        cell_id: accepted_cell_id,
        content_items,
        error_text,
    } = accepted
    else {
        panic!("expected accepted output to complete");
    };
    assert_eq!(accepted_cell_id, cell_id("1"));
    assert_eq!(error_text, None);
    let [FunctionCallOutputContentItem::InputText { text }] = content_items.as_slice() else {
        panic!("expected one text output item");
    };
    assert_eq!(text.len(), accepted_output_bytes);

    let oversized_output_bytes = MAX_FRAME_BYTES + 1024;
    let started = session
        .execute(execute_request(&format!(
            r#"text("x".repeat({oversized_output_bytes}));"#
        )))
        .await
        .expect("start oversized output");
    let error = tokio::time::timeout(Duration::from_secs(15), started.initial_response())
        .await
        .expect("oversized response timeout")
        .expect_err("oversized response should error the cell");
    assert!(
        error.contains(&format!("{MAX_FRAME_BYTES}-byte IPC frame limit")),
        "unexpected error: {error}"
    );

    assert_eq!(
        execute(&session, execute_request(r#"text("recovered");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("3"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "recovered".to_string(),
            }],
            error_text: None,
        }
    );
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn cpu_bound_output_loop_can_be_terminated() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    );
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");
    let mut request = execute_request(
        r#"
for (;;) {
    text("lots of output");
}
"#,
    );
    request.yield_time_ms = Some(1);
    let started = session.execute(request).await.expect("start output loop");
    let running_cell_id = started.cell_id.clone();
    let initial_response = tokio::time::timeout(Duration::from_secs(5), async {
        started.initial_response().await
    })
    .await
    .expect("initial response timeout")
    .expect("initial response");
    let RuntimeResponse::Yielded {
        cell_id: yielded_cell_id,
        mut content_items,
    } = initial_response
    else {
        panic!("expected the output loop to yield");
    };
    assert_eq!(yielded_cell_id, running_cell_id);
    if content_items.is_empty() {
        content_items = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let outcome = session
                    .wait(WaitRequest {
                        cell_id: running_cell_id.clone(),
                        yield_time_ms: 100,
                    })
                    .await
                    .expect("wait for output loop");
                let WaitOutcome::LiveCell(RuntimeResponse::Yielded {
                    cell_id,
                    content_items,
                }) = outcome
                else {
                    panic!("expected the output loop to remain live: {outcome:?}");
                };
                assert_eq!(cell_id, running_cell_id);
                if !content_items.is_empty() {
                    break content_items;
                }
            }
        })
        .await
        .expect("output timeout");
    }
    assert!(content_items.iter().all(|item| {
        item == &FunctionCallOutputContentItem::InputText {
            text: "lots of output".to_string(),
        }
    }));

    let termination = tokio::time::timeout(
        Duration::from_secs(5),
        session.terminate(running_cell_id.clone()),
    )
    .await
    .expect("termination timeout")
    .expect("terminate output loop");
    let WaitOutcome::LiveCell(RuntimeResponse::Terminated {
        cell_id: terminated_cell_id,
        content_items,
    }) = termination
    else {
        panic!("expected the output loop to terminate");
    };
    assert_eq!(terminated_cell_id, running_cell_id);
    assert!(content_items.iter().all(|item| {
        item == &FunctionCallOutputContentItem::InputText {
            text: "lots of output".to_string(),
        }
    }));
    session.shutdown().await.expect("shutdown remote session");
}

#[tokio::test]
async fn explicit_yield_frame_precedes_notification_and_terminal_output_drops_timers() {
    let mut child = Command::new(
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary"),
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .kill_on_drop(true)
    .spawn()
    .expect("spawn host");
    let mut writer = FramedWriter::new(child.stdin.take().expect("host stdin"));
    let mut reader = FramedReader::new(child.stdout.take().expect("host stdout"));
    writer
        .write(&ClientToHost::ClientHello(
            ClientHello::new(
                SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                    .expect("supported versions"),
                CapabilitySet::empty(),
                CapabilitySet::empty(),
            )
            .expect("client hello"),
        ))
        .await
        .expect("write client hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("host hello"),
        Some(HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            CapabilitySet::empty(),
        )))
    );

    let session_id = SessionId::new("session-1").expect("session ID");
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 1),
            request: HostRequest::OpenSession {
                session_id: session_id.clone(),
            },
        })
        .await
        .expect("open session");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("session ready"),
        Some(HostToClient::Response {
            id: request_id(/*value*/ 1),
            result: WireResult::Ok {
                value: HostResponse::SessionReady {
                    session_id: session_id.clone(),
                },
            },
        })
    );

    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 2),
            request: HostRequest::Execute {
                session_id: session_id.clone(),
                request: execute_request(
                    r#"
text("hello");
yield_control();
text("world");
notify("this is important");
setTimeout(() => { text("should never emit"); }, 60000);
"#,
                )
                .try_into()
                .expect("wire execute request"),
            },
        })
        .await
        .expect("execute request");
    assert_eq!(
        reader
            .read::<HostToClient>()
            .await
            .expect("execution started"),
        Some(HostToClient::Response {
            id: request_id(/*value*/ 2),
            result: WireResult::Ok {
                value: HostResponse::ExecutionStarted {
                    cell_id: cell_id("1").into(),
                },
            },
        })
    );
    assert_eq!(
        reader
            .read::<HostToClient>()
            .await
            .expect("initial response"),
        Some(HostToClient::InitialResponse {
            id: request_id(/*value*/ 2),
            result: WireResult::Ok {
                value: RuntimeResponse::Yielded {
                    cell_id: cell_id("1"),
                    content_items: vec![FunctionCallOutputContentItem::InputText {
                        text: "hello".to_string(),
                    }],
                }
                .into(),
            },
        })
    );

    let delegate_request_id = match reader
        .read::<HostToClient>()
        .await
        .expect("notification request")
    {
        Some(HostToClient::DelegateRequest {
            id,
            session_id: callback_session_id,
            request:
                DelegateRequest::Notify {
                    call_id,
                    cell_id: callback_cell_id,
                    text,
                },
        }) => {
            assert_eq!(callback_session_id, session_id);
            assert_eq!(call_id, "call-1");
            assert_eq!(callback_cell_id, cell_id("1").into());
            assert_eq!(text, "this is important");
            id
        }
        message => panic!("unexpected notification frame: {message:?}"),
    };
    writer
        .write(&ClientToHost::DelegateResponse {
            id: delegate_request_id,
            result: WireResult::Ok {
                value: DelegateResponse::NotificationDelivered,
            },
        })
        .await
        .expect("notification response");
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 3),
            request: HostRequest::Wait {
                session_id: session_id.clone(),
                request: WaitRequest {
                    cell_id: cell_id("1"),
                    yield_time_ms: 60_000,
                }
                .into(),
            },
        })
        .await
        .expect("wait request");

    let terminal = loop {
        match reader
            .read::<HostToClient>()
            .await
            .expect("terminal response")
        {
            Some(HostToClient::Response { id, result }) if id == request_id(/*value*/ 3) => {
                break result;
            }
            Some(HostToClient::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            }) => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(closed_cell_id, cell_id("1").into());
            }
            message => panic!("unexpected terminal frame: {message:?}"),
        }
    };
    assert_eq!(
        terminal,
        WireResult::Ok {
            value: HostResponse::WaitCompleted {
                outcome: WaitOutcome::LiveCell(RuntimeResponse::Result {
                    cell_id: cell_id("1"),
                    content_items: vec![FunctionCallOutputContentItem::InputText {
                        text: "world".to_string(),
                    }],
                    error_text: None,
                })
                .into(),
            },
        }
    );

    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 4),
            request: HostRequest::ShutdownSession {
                session_id: session_id.clone(),
            },
        })
        .await
        .expect("shutdown request");
    loop {
        match reader
            .read::<HostToClient>()
            .await
            .expect("shutdown response")
        {
            Some(HostToClient::Response { id, result }) if id == request_id(/*value*/ 4) => {
                assert_eq!(
                    result,
                    WireResult::Ok {
                        value: HostResponse::SessionClosed {
                            session_id: session_id.clone(),
                        },
                    }
                );
                break;
            }
            Some(HostToClient::CellClosed { .. }) => {}
            message => panic!("unexpected shutdown frame: {message:?}"),
        }
    }
    drop(writer);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("host exit timeout")
        .expect("wait for host");
    assert!(status.success(), "host exited with {status}");
}

#[cfg(unix)]
#[tokio::test]
async fn closed_host_stdout_terminates_the_spawned_process() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let pid_log = temp_dir.path().join("host-pids");
    let wrapper = temp_dir.path().join("stdout-closing-host");
    let payload = serde_json::to_vec(&HostToClient::HostHello(HostHello::new(
        ProtocolVersion::V1,
        CapabilitySet::empty(),
    )))
    .expect("serialize host hello");
    let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
    frame.extend(payload);
    let escaped_frame = frame
        .iter()
        .map(|byte| format!("\\{byte:03o}"))
        .collect::<String>();
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$$\" >> {}\nprintf '%b' '{escaped_frame}'\nexec 1>&-\nwhile :; do :; done\n",
        shell_quote(&pid_log),
    );
    std::fs::write(&wrapper, script).expect("write host wrapper");
    let mut permissions = std::fs::metadata(&wrapper)
        .expect("host wrapper metadata")
        .permissions();
    permissions.set_mode(/*mode*/ 0o755);
    std::fs::set_permissions(&wrapper, permissions).expect("make host wrapper executable");

    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(wrapper);
    let session_result = tokio::time::timeout(
        Duration::from_secs(5),
        provider.create_session(Arc::new(RecordingDelegate::default())),
    )
    .await;
    let pid = recorded_pids(&pid_log)[0];
    let error = match session_result {
        Ok(result) => result
            .err()
            .expect("closed stdout should fail session creation"),
        Err(error) => {
            // SAFETY: `pid` was written by the wrapper process owned by this test.
            let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
            panic!("session creation timeout: {error}");
        }
    };
    assert!(
        error.contains("closed its stdout"),
        "unexpected error: {error}"
    );

    let exited = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            // SAFETY: signal zero only checks whether the process recorded by
            // this test still exists.
            if unsafe {
                libc::kill(pid, /*sig*/ 0)
            } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if let Err(error) = exited {
        // SAFETY: `pid` was written by the wrapper process owned by this test.
        let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
        panic!("host process was not terminated: {error}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn crashed_host_fails_in_flight_exec_and_next_exec_respawns() {
    use std::os::unix::fs::PermissionsExt;

    let host_binary =
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("host binary");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let pid_log = temp_dir.path().join("host-pids");
    let wrapper = temp_dir.path().join("code-mode-host-wrapper");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$$\" >> {}\nexec {}\n",
        shell_quote(&pid_log),
        shell_quote(&host_binary),
    );
    std::fs::write(&wrapper, script).expect("write host wrapper");
    let mut permissions = std::fs::metadata(&wrapper)
        .expect("host wrapper metadata")
        .permissions();
    permissions.set_mode(/*mode*/ 0o755);
    std::fs::set_permissions(&wrapper, permissions).expect("make host wrapper executable");

    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(wrapper);
    let session = provider
        .create_session(Arc::new(RecordingDelegate::default()))
        .await
        .expect("create remote session");
    let mut request = execute_request("await new Promise(() => {});");
    request.yield_time_ms = Some(60_000);
    let started = session.execute(request).await.expect("start execution");
    let first_pid = recorded_pids(&pid_log)[0];

    // SAFETY: `first_pid` was written by the wrapper immediately before it
    // replaced itself with the host process owned by this test.
    assert_eq!(unsafe { libc::kill(first_pid, libc::SIGKILL) }, 0);
    let error = tokio::time::timeout(Duration::from_secs(5), started.initial_response())
        .await
        .expect("host crash propagation timeout")
        .expect_err("crashed host should fail the in-flight execution");
    assert!(
        error.contains("code-mode host"),
        "unexpected error: {error}"
    );

    assert_eq!(
        execute(&session, execute_request(r#"text("recovered");"#)).await,
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "recovered".to_string(),
            }],
            error_text: None,
        }
    );
    let pids = recorded_pids(&pid_log);
    assert_eq!(pids.len(), 2);
    assert_ne!(pids[0], pids[1]);
    session.shutdown().await.expect("shutdown remote session");
}

#[cfg(unix)]
fn shell_quote(path: &std::path::Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

#[cfg(unix)]
fn recorded_pids(path: &std::path::Path) -> Vec<libc::pid_t> {
    std::fs::read_to_string(path)
        .expect("read host pid log")
        .lines()
        .map(|line| line.parse().expect("parse host pid"))
        .collect()
}
