#![allow(clippy::expect_used)]

use std::process::Stdio;
use std::time::Duration;

use codex_code_mode_protocol::wire::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::Command;
use tokio::sync::mpsc;

struct HostHarness {
    child: Child,
    stdin: Option<ChildStdin>,
    messages_rx: mpsc::UnboundedReceiver<Result<HostMessage, String>>,
}

impl HostHarness {
    fn spawn() -> Self {
        let mut child = Command::new(
            codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("resolve host binary"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn host");
        let stdin = child.stdin.take().expect("host stdin");
        let mut stdout = child.stdout.take().expect("host stdout");
        let (messages_tx, messages_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                match read_frame(&mut stdout).await {
                    Ok(Some(message)) => {
                        if messages_tx.send(Ok(message)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = messages_tx.send(Err(err.to_string()));
                        break;
                    }
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            messages_rx,
        }
    }

    async fn send(&mut self, message: ClientMessage) {
        write_frame(self.stdin.as_mut().expect("host stdin open"), &message)
            .await
            .expect("write host message");
    }

    async fn recv(&mut self) -> HostMessage {
        tokio::time::timeout(Duration::from_secs(5), self.messages_rx.recv())
            .await
            .expect("host message timeout")
            .expect("host output closed")
            .expect("read host message")
    }

    async fn assert_no_message(&mut self, duration: Duration) {
        assert!(
            tokio::time::timeout(duration, self.messages_rx.recv())
                .await
                .is_err(),
            "received an unexpected host message"
        );
    }

    async fn create_session(&mut self) -> SessionId {
        self.send(ClientMessage::Request {
            id: 1,
            request: HostRequest::CreateSession,
        })
        .await;
        match self.recv().await {
            HostMessage::Response {
                id: 1,
                result:
                    WireResult::Ok {
                        value: HostResponse::SessionCreated { session_id },
                    },
            } => session_id,
            message => panic!("unexpected create-session response: {message:?}"),
        }
    }

    async fn shutdown_session(&mut self, session_id: SessionId, request_id: RequestId) {
        self.send(ClientMessage::Request {
            id: request_id,
            request: HostRequest::ShutdownSession { session_id },
        })
        .await;
        assert_eq!(
            self.recv().await,
            HostMessage::Response {
                id: request_id,
                result: WireResult::Ok {
                    value: HostResponse::SessionShutdown,
                },
            }
        );
    }

    async fn finish(mut self) {
        self.stdin.take();
        let status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .expect("host exit timeout")
            .expect("wait for host");
        assert!(status.success(), "host exited with {status}");
    }
}

fn create_cell_request(source: &str) -> CreateCellRequest {
    CreateCellRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

async fn create_cell(
    host: &mut HostHarness,
    session_id: SessionId,
    request_id: RequestId,
    request: CreateCellRequest,
) -> CellId {
    host.send(ClientMessage::Request {
        id: request_id,
        request: HostRequest::CreateCell {
            session_id,
            request,
        },
    })
    .await;
    match host.recv().await {
        HostMessage::Response {
            id,
            result:
                WireResult::Ok {
                    value: HostResponse::CellCreated { cell_id },
                },
        } if id == request_id => cell_id,
        message => panic!("unexpected create-cell response: {message:?}"),
    }
}

async fn observe_cell(
    host: &mut HostHarness,
    session_id: SessionId,
    request_id: RequestId,
    cell_id: CellId,
    mode: ObserveMode,
) {
    host.send(ClientMessage::Request {
        id: request_id,
        request: HostRequest::Observe {
            session_id,
            cell_id,
            mode,
        },
    })
    .await;
}

async fn recv_observation(host: &mut HostHarness, request_id: RequestId) -> CellEvent {
    match host.recv().await {
        HostMessage::Response {
            id,
            result:
                WireResult::Ok {
                    value: HostResponse::Observed { event },
                },
        } if id == request_id => event,
        message => panic!("unexpected observation response: {message:?}"),
    }
}

async fn recv_terminal_observation(
    host: &mut HostHarness,
    session_id: SessionId,
    request_id: RequestId,
    cell_id: &CellId,
) -> CellEvent {
    let mut event = None;
    let mut cell_closed = false;
    for _ in 0..2 {
        match host.recv().await {
            HostMessage::Response {
                id,
                result:
                    WireResult::Ok {
                        value: HostResponse::Observed { event: response },
                    },
            } if id == request_id => event = Some(response),
            HostMessage::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            } => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(&closed_cell_id, cell_id);
                cell_closed = true;
            }
            message => panic!("unexpected terminal observation message: {message:?}"),
        }
    }
    assert!(cell_closed);
    event.expect("terminal observation response")
}

#[tokio::test]
async fn yields_resumes_and_closes_over_stdio() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let cell_id = create_cell(
        &mut host,
        session_id,
        2,
        create_cell_request(concat!(
            "await new Promise(resolve => setTimeout(resolve, 100));",
            r#"text("before"); yield_control(); text("after");"#,
        )),
    )
    .await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id.clone(),
        ObserveMode::YieldAfter {
            duration_ms: 60_000,
        },
    )
    .await;
    let mut yielded = None;
    let mut saw_cell_closed = false;
    for _ in 0..2 {
        match host.recv().await {
            HostMessage::Response {
                id: 3,
                result:
                    WireResult::Ok {
                        value: HostResponse::Observed { event },
                    },
            } => yielded = Some(event),
            HostMessage::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            } => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(closed_cell_id, cell_id);
                saw_cell_closed = true;
            }
            message => panic!("unexpected initial observation message: {message:?}"),
        }
    }
    assert_eq!(
        yielded,
        Some(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        })
    );
    assert!(saw_cell_closed);
    observe_cell(
        &mut host,
        session_id,
        4,
        cell_id.clone(),
        ObserveMode::YieldAfter { duration_ms: 1 },
    )
    .await;
    assert_eq!(
        recv_observation(&mut host, 4).await,
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "after".to_string(),
            }],
            error_text: None,
        }
    );
    host.shutdown_session(session_id, 5).await;
    host.finish().await;
}

#[tokio::test]
async fn termination_resolves_an_active_observer() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let cell_id = create_cell(
        &mut host,
        session_id,
        2,
        create_cell_request("await new Promise(() => {});"),
    )
    .await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id.clone(),
        ObserveMode::YieldAfter {
            duration_ms: 60_000,
        },
    )
    .await;
    host.assert_no_message(Duration::from_millis(100)).await;

    host.send(ClientMessage::Request {
        id: 4,
        request: HostRequest::Terminate {
            session_id,
            cell_id: cell_id.clone(),
        },
    })
    .await;

    let mut saw_observation_response = false;
    let mut saw_termination_response = false;
    let mut saw_cell_closed = false;
    for _ in 0..3 {
        match host.recv().await {
            HostMessage::Response {
                id: 3,
                result:
                    WireResult::Ok {
                        value:
                            HostResponse::Observed {
                                event: CellEvent::Terminated { content_items },
                            },
                    },
            } => {
                assert!(content_items.is_empty());
                saw_observation_response = true;
            }
            HostMessage::Response {
                id: 4,
                result:
                    WireResult::Ok {
                        value:
                            HostResponse::Observed {
                                event: CellEvent::Terminated { content_items },
                            },
                    },
            } => {
                assert!(content_items.is_empty());
                saw_termination_response = true;
            }
            HostMessage::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            } => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(closed_cell_id, cell_id);
                saw_cell_closed = true;
            }
            message => panic!("unexpected termination message: {message:?}"),
        }
    }
    assert!(saw_observation_response);
    assert!(saw_termination_response);
    assert!(saw_cell_closed);

    host.shutdown_session(session_id, 5).await;
    host.finish().await;
}

#[tokio::test]
async fn forwards_tool_and_notification_callbacks() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let mut request = create_cell_request(
        r#"
notify("note");
const value = await tools.echo({ value: "input" });
text(value.value);
"#,
    );
    request.enabled_tools.push(ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName {
            name: "echo".to_string(),
            namespace: None,
        },
        description: String::new(),
        kind: ToolKind::Function,
    });
    let cell_id = create_cell(&mut host, session_id, 2, request).await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id.clone(),
        ObserveMode::YieldAfter {
            duration_ms: 60_000,
        },
    )
    .await;

    for _ in 0..2 {
        match host.recv().await {
            HostMessage::CallbackRequest {
                id,
                session_id: callback_session_id,
                request: CallbackRequest::Notify { text, .. },
            } => {
                assert_eq!(callback_session_id, session_id);
                assert_eq!(text, "note");
                host.send(ClientMessage::CallbackResponse {
                    id,
                    response: CallbackResponse::NotificationDelivered,
                })
                .await;
            }
            HostMessage::CallbackRequest {
                id,
                session_id: callback_session_id,
                request: CallbackRequest::InvokeTool { invocation },
            } => {
                assert_eq!(callback_session_id, session_id);
                assert_eq!(invocation.input, Some(json!({"value": "input"})));
                host.send(ClientMessage::CallbackResponse {
                    id,
                    response: CallbackResponse::ToolResult {
                        result: json!({"value": "output"}),
                    },
                })
                .await;
            }
            message => panic!("unexpected callback message: {message:?}"),
        }
    }

    assert_eq!(
        recv_terminal_observation(&mut host, session_id, 3, &cell_id).await,
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "output".to_string(),
            }],
            error_text: None,
        }
    );
    host.shutdown_session(session_id, 4).await;
    host.finish().await;
}

#[tokio::test]
async fn pending_frontier_rejects_a_second_observer_and_termination_preempts_the_first() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let cell_id = create_cell(
        &mut host,
        session_id,
        2,
        create_cell_request("await new Promise(() => {});"),
    )
    .await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id.clone(),
        ObserveMode::PendingFrontier,
    )
    .await;
    assert_eq!(
        recv_observation(&mut host, 3).await,
        CellEvent::Pending {
            content_items: Vec::new(),
            pending_tool_call_ids: Vec::new(),
        }
    );

    host.send(ClientMessage::Request {
        id: 4,
        request: HostRequest::Observe {
            session_id,
            cell_id: cell_id.clone(),
            mode: ObserveMode::YieldAfter {
                duration_ms: 60_000,
            },
        },
    })
    .await;
    host.assert_no_message(Duration::from_millis(100)).await;
    host.send(ClientMessage::Request {
        id: 5,
        request: HostRequest::Observe {
            session_id,
            cell_id: cell_id.clone(),
            mode: ObserveMode::YieldAfter {
                duration_ms: 60_000,
            },
        },
    })
    .await;
    assert_eq!(
        host.recv().await,
        HostMessage::Response {
            id: 5,
            result: WireResult::Err {
                error: Error::BusyObserver {
                    cell_id: cell_id.clone(),
                },
            },
        }
    );

    host.send(ClientMessage::Request {
        id: 6,
        request: HostRequest::Terminate {
            session_id,
            cell_id: cell_id.clone(),
        },
    })
    .await;
    let terminated = HostResponse::Observed {
        event: CellEvent::Terminated {
            content_items: Vec::new(),
        },
    };
    let mut response_ids = Vec::new();
    let mut saw_cell_closed = false;
    for _ in 0..3 {
        match host.recv().await {
            HostMessage::Response {
                id,
                result: WireResult::Ok { value },
            } => {
                assert_eq!(value, terminated);
                response_ids.push(id);
            }
            HostMessage::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            } => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(closed_cell_id, cell_id);
                saw_cell_closed = true;
            }
            message => panic!("unexpected termination response: {message:?}"),
        }
    }
    response_ids.sort_unstable();
    assert_eq!(response_ids, vec![4, 6]);
    assert!(saw_cell_closed);
    host.shutdown_session(session_id, 7).await;
    host.finish().await;
}

#[tokio::test]
async fn shutdown_cancels_callbacks_before_acknowledging_the_session() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let mut request = create_cell_request("await tools.echo({});");
    request.enabled_tools.push(ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName {
            name: "echo".to_string(),
            namespace: None,
        },
        description: String::new(),
        kind: ToolKind::Function,
    });
    let cell_id = create_cell(&mut host, session_id, 2, request).await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id.clone(),
        ObserveMode::YieldAfter {
            duration_ms: 60_000,
        },
    )
    .await;
    let callback_id = match host.recv().await {
        HostMessage::CallbackRequest {
            id,
            request: CallbackRequest::InvokeTool { .. },
            ..
        } => id,
        message => panic!("unexpected callback message: {message:?}"),
    };

    host.send(ClientMessage::Request {
        id: 4,
        request: HostRequest::ShutdownSession { session_id },
    })
    .await;
    let mut saw_callback_cancellation = false;
    let mut saw_terminated_response = false;
    let mut saw_shutdown_response = false;
    let mut saw_cell_closed = false;
    for _ in 0..4 {
        match host.recv().await {
            HostMessage::CancelCallback { id } => {
                assert_eq!(id, callback_id);
                saw_callback_cancellation = true;
            }
            HostMessage::Response {
                id: 3,
                result:
                    WireResult::Ok {
                        value:
                            HostResponse::Observed {
                                event: CellEvent::Terminated { content_items },
                            },
                    },
            } => {
                assert!(content_items.is_empty());
                saw_terminated_response = true;
            }
            HostMessage::Response {
                id: 4,
                result:
                    WireResult::Ok {
                        value: HostResponse::SessionShutdown,
                    },
            } => saw_shutdown_response = true,
            HostMessage::CellClosed {
                session_id: closed_session_id,
                cell_id: closed_cell_id,
            } => {
                assert_eq!(closed_session_id, session_id);
                assert_eq!(closed_cell_id, cell_id);
                saw_cell_closed = true;
            }
            message => panic!("unexpected shutdown message: {message:?}"),
        }
    }
    assert!(saw_callback_cancellation);
    assert!(saw_terminated_response);
    assert!(saw_shutdown_response);
    assert!(saw_cell_closed);
    host.finish().await;
}

#[tokio::test]
async fn disconnect_exits_with_a_pending_callback() {
    let mut host = HostHarness::spawn();
    let session_id = host.create_session().await;
    let mut request = create_cell_request("await tools.echo({});");
    request.enabled_tools.push(ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName {
            name: "echo".to_string(),
            namespace: None,
        },
        description: String::new(),
        kind: ToolKind::Function,
    });
    let cell_id = create_cell(&mut host, session_id, 2, request).await;
    observe_cell(
        &mut host,
        session_id,
        3,
        cell_id,
        ObserveMode::YieldAfter {
            duration_ms: 60_000,
        },
    )
    .await;
    assert!(matches!(
        host.recv().await,
        HostMessage::CallbackRequest {
            request: CallbackRequest::InvokeTool { .. },
            ..
        }
    ));

    host.finish().await;
}
