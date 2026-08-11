use super::GrpcCodeModeHost;
use super::GrpcStream;
use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHost;
use futures::FutureExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tonic::Code;
use tonic::Request;

pub(super) async fn open_session(
    host: &GrpcCodeModeHost,
) -> (String, GrpcStream<proto::SessionEvent>) {
    let mut stream = host
        .open_session(Request::new(proto::OpenSessionRequest {
            cell_execution_limits: None,
        }))
        .await
        .expect("open code-mode session")
        .into_inner();
    let event = stream
        .next()
        .await
        .expect("session opened event")
        .expect("session event");
    let Some(proto::session_event::Event::Opened(opened)) = event.event else {
        panic!("expected the first session event to open its lease");
    };
    (opened.session_id, stream)
}

pub(super) fn execute_request(
    session_id: &str,
    execution_id: &str,
    source: &str,
) -> proto::ExecuteRequest {
    proto::ExecuteRequest {
        session_id: session_id.to_string(),
        execution_id: execution_id.to_string(),
        tool_call_id: "outer-call".to_string(),
        source: source.to_string(),
        enabled_tools: Vec::new(),
        yield_time_ms: Some(1),
        max_output_tokens: None,
    }
}

pub(super) fn tool(name: &str) -> proto::ToolDefinition {
    proto::ToolDefinition {
        name: name.to_string(),
        tool_name: Some(proto::ToolName {
            name: name.to_string(),
            namespace: None,
        }),
        description: String::new(),
        kind: proto::ToolKind::Function as i32,
        input_schema_json: None,
        output_schema_json: None,
    }
}

pub(super) async fn execute_events(
    host: &GrpcCodeModeHost,
    request: proto::ExecuteRequest,
) -> (String, GrpcStream<proto::ExecuteEvent>) {
    let mut stream = host
        .execute(Request::new(request))
        .await
        .expect("execute cell")
        .into_inner();
    let event = stream
        .next()
        .await
        .expect("execution started event")
        .expect("execution event");
    let Some(proto::execute_event::Event::Started(started)) = event.event else {
        panic!("expected execution admission before its outcome");
    };
    (started.cell_id, stream)
}

#[tokio::test]
async fn execute_stream_starts_immediately_and_wait_preserves_missing_cells() {
    let host = GrpcCodeModeHost::new();
    let (session_id, mut session_events) = open_session(&host).await;
    let mut request = execute_request(
        &session_id,
        "execution-1",
        r#"text("before"); yield_control(); text("after");"#,
    );
    request.yield_time_ms = Some(60_000);
    let (cell_id, mut execution) = execute_events(&host, request).await;

    assert_eq!(
        execution.next().await.unwrap().unwrap(),
        proto::ExecuteEvent {
            event: Some(proto::execute_event::Event::Outcome(
                proto::ExecutionOutcome {
                    cell_id: cell_id.clone(),
                    content_items: vec![proto::ContentItem {
                        item: Some(proto::content_item::Item::Text(proto::TextContent {
                            text: "before".to_string(),
                        })),
                    }],
                    outcome: Some(proto::execution_outcome::Outcome::Yielded(
                        proto::ExecutionYielded {},
                    )),
                }
            )),
        }
    );
    let completed = host
        .wait(Request::new(proto::WaitRequest {
            session_id: session_id.clone(),
            cell_id: cell_id.clone(),
            wait_id: "wait-1".to_string(),
            yield_time_ms: 60_000,
        }))
        .await
        .expect("wait for completion")
        .into_inner();
    assert_eq!(
        completed,
        proto::WaitResponse {
            state: Some(proto::wait_response::State::LiveCell(
                proto::ExecutionOutcome {
                    cell_id: cell_id.clone(),
                    content_items: vec![proto::ContentItem {
                        item: Some(proto::content_item::Item::Text(proto::TextContent {
                            text: "after".to_string(),
                        })),
                    }],
                    outcome: Some(proto::execution_outcome::Outcome::Completed(
                        proto::ExecutionCompleted { error_text: None },
                    )),
                }
            )),
        }
    );
    assert_eq!(
        session_events.next().await.unwrap().unwrap(),
        proto::SessionEvent {
            event: Some(proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: "execution-1".to_string(),
                cell_id: cell_id.clone(),
                final_tool_call_sequence: 0,
            })),
        }
    );

    let missing = host
        .wait(Request::new(proto::WaitRequest {
            session_id,
            cell_id: "missing-cell".to_string(),
            wait_id: "wait-missing".to_string(),
            yield_time_ms: 1,
        }))
        .await
        .expect("missing cells remain successful wait outcomes")
        .into_inner();
    assert!(matches!(
        missing.state,
        Some(proto::wait_response::State::MissingCell(_))
    ));
}

#[tokio::test]
async fn filtered_subscriptions_receive_ordered_calls_and_unary_completions() {
    let host = GrpcCodeModeHost::new();
    let (session_id, mut session_events) = open_session(&host).await;
    let mut matching = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![proto::ToolName {
                name: "echo".to_string(),
                namespace: None,
            }],
        }))
        .await
        .unwrap()
        .into_inner();
    let mut unrelated = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![proto::ToolName {
                name: "other".to_string(),
                namespace: None,
            }],
        }))
        .await
        .unwrap()
        .into_inner();
    let mut request = execute_request(
        &session_id,
        "execution-tools",
        r#"text(await tools.echo({value: 1})); text(await tools.echo({value: 2}));"#,
    );
    request.yield_time_ms = Some(60_000);
    request.enabled_tools = vec![tool("echo")];
    let (cell_id, mut execution) = execute_events(&host, request).await;

    for sequence in [1, 2] {
        let invocation = matching.next().await.unwrap().unwrap();
        assert_eq!(invocation.session_id, session_id);
        assert_eq!(invocation.execution_id, "execution-tools");
        assert_eq!(invocation.cell_id, cell_id);
        assert_eq!(invocation.sequence, sequence);
        assert_eq!(
            invocation.input_json,
            Some(format!(r#"{{"value":{sequence}}}"#).into_bytes())
        );
        assert!(unrelated.next().now_or_never().is_none());
        host.complete_tool_call(Request::new(proto::CompleteToolCallRequest {
            session_id: session_id.clone(),
            invocation_id: invocation.invocation_id,
            outcome: Some(proto::complete_tool_call_request::Outcome::Succeeded(
                proto::ToolCallSucceeded {
                    output_json: format!(r#""result-{sequence}""#).into_bytes(),
                },
            )),
        }))
        .await
        .expect("complete delegated tool");
    }

    let outcome = execution.next().await.unwrap().unwrap();
    assert!(matches!(
        outcome.event,
        Some(proto::execute_event::Event::Outcome(
            proto::ExecutionOutcome {
                outcome: Some(proto::execution_outcome::Outcome::Completed(_)),
                ..
            }
        ))
    ));
    assert_eq!(
        session_events.next().await.unwrap().unwrap(),
        proto::SessionEvent {
            event: Some(proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: "execution-tools".to_string(),
                cell_id,
                final_tool_call_sequence: 2,
            })),
        }
    );
}

#[tokio::test]
async fn cancellation_before_wait_admission_is_preserved() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-wait",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.unwrap().unwrap();

    host.cancel_wait(Request::new(proto::CancelWaitRequest {
        session_id: session_id.clone(),
        wait_id: "pre-cancelled".to_string(),
    }))
    .await
    .expect("record cancellation before wait admission");
    let cancelled = host
        .wait(Request::new(proto::WaitRequest {
            session_id: session_id.clone(),
            cell_id: cell_id.clone(),
            wait_id: "pre-cancelled".to_string(),
            yield_time_ms: 60_000,
        }))
        .await
        .unwrap_err();
    assert_eq!(cancelled.code(), Code::Cancelled);

    host.terminate(Request::new(proto::TerminateRequest {
        session_id,
        cell_id,
    }))
    .await
    .unwrap();
}

#[tokio::test]
async fn notifications_do_not_delay_cell_completion() {
    let host = GrpcCodeModeHost::new();
    let (session_id, mut session_events) = open_session(&host).await;
    let mut request = execute_request(
        &session_id,
        "execution-notify",
        r#"notify("pending"); text("done");"#,
    );
    request.yield_time_ms = Some(60_000);
    let (cell_id, mut execution) = execute_events(&host, request).await;
    let event = session_events.next().await.unwrap().unwrap();
    let Some(proto::session_event::Event::Notification(notification)) = event.event else {
        panic!("expected pending notification");
    };
    assert_eq!(notification.execution_id, "execution-notify");
    assert_eq!(notification.cell_id, cell_id);
    assert_eq!(notification.call_id, "outer-call");
    assert_eq!(notification.text, "pending");
    assert!(matches!(
        execution.next().await.unwrap().unwrap().event,
        Some(proto::execute_event::Event::Outcome(
            proto::ExecutionOutcome {
                outcome: Some(proto::execution_outcome::Outcome::Completed(_)),
                ..
            }
        ))
    ));
    assert_eq!(
        session_events.next().await.unwrap().unwrap(),
        proto::SessionEvent {
            event: Some(proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: "execution-notify".to_string(),
                cell_id,
                final_tool_call_sequence: 0,
            })),
        }
    );
    host.acknowledge_notification(Request::new(proto::AcknowledgeNotificationRequest {
        session_id,
        notification_id: notification.notification_id,
    }))
    .await
    .expect("legacy notification acknowledgments remain accepted");
}
