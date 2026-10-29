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
    let (opened, stream) = open_session_with_truncation(host, /*output_truncation*/ None).await;
    (opened.session_id, stream)
}

async fn open_session_with_truncation(
    host: &GrpcCodeModeHost,
    output_truncation: Option<i32>,
) -> (proto::SessionOpened, GrpcStream<proto::SessionEvent>) {
    let mut stream = host
        .open_session(Request::new(proto::OpenSessionRequest {
            cell_execution_limits: None,
            output_truncation,
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
    (opened, stream)
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

fn only_text(items: &[proto::ContentItem]) -> &str {
    let [
        proto::ContentItem {
            item: Some(proto::content_item::Item::Text(text)),
        },
    ] = items
    else {
        panic!("expected one text item, got {items:?}");
    };
    &text.text
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

    let yielded = execution.next().await.unwrap().unwrap();
    let Some(proto::execute_event::Event::Outcome(yielded_outcome)) = &yielded.event else {
        panic!("expected execution outcome");
    };
    assert_eq!(
        yielded,
        proto::ExecuteEvent {
            event: Some(proto::execute_event::Event::Outcome(
                proto::ExecutionOutcome {
                    code_mode_host_duration_ns: yielded_outcome.code_mode_host_duration_ns,
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
            max_output_tokens: None,
        }))
        .await
        .expect("wait for completion")
        .into_inner();
    let Some(proto::wait_response::State::LiveCell(completed_outcome)) = &completed.state else {
        panic!("expected live-cell wait outcome");
    };
    assert_eq!(
        completed,
        proto::WaitResponse {
            state: Some(proto::wait_response::State::LiveCell(
                proto::ExecutionOutcome {
                    code_mode_host_duration_ns: completed_outcome.code_mode_host_duration_ns,
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
            max_output_tokens: None,
        }))
        .await
        .expect("missing cells remain successful wait outcomes")
        .into_inner();
    assert!(matches!(
        missing.state,
        Some(proto::wait_response::State::MissingCell(_))
    ));
}

/// Missing, empty, and explicit default namespaces identify the same subscribed tool.
#[tokio::test]
async fn filtered_subscriptions_match_default_namespace_aliases() {
    let aliases = [
        (None, Some("functions".to_string())),
        (Some(String::new()), None),
        (Some("functions".to_string()), Some(String::new())),
    ];

    for (tool_namespace, filter_namespace) in aliases {
        let host = GrpcCodeModeHost::new();
        let (session_id, _events) = open_session(&host).await;
        let mut subscription = host
            .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
                session_id: session_id.clone(),
                tool_names: vec![proto::ToolName {
                    name: "echo".to_string(),
                    namespace: filter_namespace,
                }],
            }))
            .await
            .expect("subscribe with a default-namespace alias")
            .into_inner();
        let mut request = execute_request(
            &session_id,
            "default-namespace",
            "text(await tools.echo({}));",
        );
        request.yield_time_ms = Some(/*value*/ 60_000);
        let mut enabled_tool = tool("echo");
        enabled_tool.tool_name = Some(proto::ToolName {
            name: "echo".to_string(),
            namespace: tool_namespace.clone(),
        });
        request.enabled_tools = vec![enabled_tool];
        let (_cell_id, mut execution) = execute_events(&host, request).await;

        let invocation = subscription
            .next()
            .await
            .expect("matching default-namespace invocation")
            .expect("tool invocation");
        assert_eq!(
            invocation.tool_name,
            Some(proto::ToolName {
                name: "echo".to_string(),
                namespace: tool_namespace,
            })
        );
        host.complete_tool_call(Request::new(proto::CompleteToolCallRequest {
            session_id,
            invocation_id: invocation.invocation_id,
            outcome: Some(proto::complete_tool_call_request::Outcome::Succeeded(
                proto::ToolCallSucceeded {
                    output_json: br#""done""#.to_vec(),
                },
            )),
        }))
        .await
        .expect("complete the matching tool call");
        assert!(matches!(
            execution.next().await,
            Some(Ok(proto::ExecuteEvent {
                event: Some(proto::execute_event::Event::Outcome(
                    proto::ExecutionOutcome {
                        outcome: Some(proto::execution_outcome::Outcome::Completed(_)),
                        ..
                    }
                )),
            }))
        ));
    }
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
            max_output_tokens: Some(0),
        }))
        .await
        .unwrap_err();
    assert_eq!(cancelled.code(), Code::Cancelled);

    host.terminate(Request::new(proto::TerminateRequest {
        session_id,
        cell_id,
        max_output_tokens: Some(0),
    }))
    .await
    .unwrap();
}

#[tokio::test]
async fn yield_before_observation_registration_is_remembered() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let session = host.state.session(&session_id).unwrap();
    for observation in [
        proto::yield_observation_request::Observation::ExecutionId("exec".to_string()),
        proto::yield_observation_request::Observation::WaitId("wait".to_string()),
    ] {
        host.yield_observation(Request::new(proto::YieldObservationRequest {
            session_id: session_id.clone(),
            observation: Some(observation),
        }))
        .await
        .unwrap();
    }

    assert!(session.reserve_execution("exec").unwrap().is_cancelled());
    let wait = super::waits::WaitRegistration::new(session, "wait".to_string()).unwrap();
    assert!(wait.yield_signal().is_cancelled());
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

#[tokio::test]
async fn output_truncation_is_a_session_setting_and_omitting_it_keeps_legacy_output() {
    for (output_truncation, bounded_output) in [
        (None, false),
        (Some(proto::OutputTruncation::Unspecified.into()), false),
        (Some(proto::OutputTruncation::Client.into()), false),
        (Some(proto::OutputTruncation::Server.into()), true),
    ] {
        let host = GrpcCodeModeHost::new();
        let (opened, _lease) = open_session_with_truncation(&host, output_truncation).await;
        assert_eq!(
            opened.output_truncation,
            bounded_output.then_some(proto::OutputTruncation::Server.into()),
        );
        let mut request = execute_request(
            &opened.session_id,
            "execution",
            "text('x'.repeat(2 * 1024 * 1024));",
        );
        request.yield_time_ms = Some(60_000);
        let (_, mut execution) = execute_events(&host, request).await;
        let outcome = execution
            .next()
            .await
            .expect("outcome event")
            .expect("outcome");
        let Some(proto::execute_event::Event::Outcome(outcome)) = outcome.event else {
            panic!("expected execution outcome");
        };
        assert!(matches!(
            outcome.outcome,
            Some(proto::execution_outcome::Outcome::Completed(_))
        ));
        assert_eq!(
            outcome.content_items.iter().any(
                |item| matches!(&item.item, Some(proto::content_item::Item::Text(text))
                    if text.text.starts_with("[Code Mode session output limit reached: omitted "))
            ),
            bounded_output
        );
        if !bounded_output {
            assert_eq!(
                only_text(&outcome.content_items),
                "x".repeat(2 * 1024 * 1024)
            );
        }
    }
}

#[tokio::test]
async fn server_output_budgets_follow_exec_wait_and_terminate_over_grpc() {
    for (execution_id, after_yield) in [
        ("failed", "throw '0123456789'.repeat(8);"),
        (
            "terminated",
            "text('0123456789'.repeat(8)); notify('ready to terminate'); await new Promise(() => {});",
        ),
    ] {
        let host = GrpcCodeModeHost::new();
        let (opened, mut lease) =
            open_session_with_truncation(&host, Some(proto::OutputTruncation::Server.into())).await;
        let source = format!("text('0123456789'.repeat(4)); yield_control(); {after_yield}");
        let mut request = execute_request(&opened.session_id, execution_id, &source);
        request.yield_time_ms = Some(60_000);
        request.max_output_tokens = Some(5);
        let (cell_id, mut execution) = execute_events(&host, request).await;
        let Some(proto::execute_event::Event::Outcome(first)) =
            execution.next().await.unwrap().unwrap().event
        else {
            panic!("expected execution outcome");
        };
        assert_eq!(first.cell_id, cell_id);
        assert!(matches!(
            first.outcome,
            Some(proto::execution_outcome::Outcome::Yielded(_))
        ));
        assert_eq!(
            only_text(&first.content_items),
            "Warning: truncated output (original token count: 10)\nTotal output lines: 1\n\n0123456789…5 tokens truncated…0123456789"
        );

        let response = if execution_id == "failed" {
            host.wait(Request::new(proto::WaitRequest {
                session_id: opened.session_id,
                cell_id: cell_id.clone(),
                wait_id: "wait-after-yield".to_string(),
                yield_time_ms: 60_000,
                max_output_tokens: Some(7),
            }))
            .await
            .expect("script failures are completed results, not RPC failures")
            .into_inner()
        } else {
            // The event is ordered after text in the runtime FIFO, so the
            // termination observes it without a sleep or racing the cell.
            assert!(matches!(
                lease.next().await.unwrap().unwrap().event,
                Some(proto::session_event::Event::Notification(
                    proto::Notification { text, .. }
                )) if text == "ready to terminate"
            ));
            host.terminate(Request::new(proto::TerminateRequest {
                session_id: opened.session_id,
                cell_id: cell_id.clone(),
                max_output_tokens: Some(7),
            }))
            .await
            .expect("terminate with an independent output budget")
            .into_inner()
        };
        let Some(proto::wait_response::State::LiveCell(last)) = response.state else {
            panic!("expected final observation from the live cell");
        };
        let expected_text = if execution_id == "failed" {
            let Some(proto::execution_outcome::Outcome::Completed(proto::ExecutionCompleted {
                error_text: Some(diagnostic),
            })) = &last.outcome
            else {
                panic!(
                    "expected completed failure with a diagnostic, got {:?}",
                    last.outcome
                );
            };
            assert_eq!(diagnostic, &"0123456789".repeat(8));
            "Warning: truncated output (original token count: 24)\nTotal output lines: 2\n\nScript error:\n…17 tokens truncated…67890123456789"
        } else {
            assert_eq!(
                last.outcome,
                Some(proto::execution_outcome::Outcome::Terminated(
                    proto::ExecutionTerminated {}
                ))
            );
            "Warning: truncated output (original token count: 20)\nTotal output lines: 1\n\n01234567890123…13 tokens truncated…67890123456789"
        };
        assert_eq!(last.cell_id, cell_id);
        assert_eq!(only_text(&last.content_items), expected_text);
    }
}

#[tokio::test]
async fn unknown_output_truncation_is_rejected_before_opening_a_session() {
    let host = GrpcCodeModeHost::new();
    let result = host
        .open_session(Request::new(proto::OpenSessionRequest {
            cell_execution_limits: None,
            output_truncation: Some(42),
        }))
        .await;
    let Err(status) = result else {
        panic!("unknown truncation mode should be rejected");
    };
    assert_eq!(status.code(), Code::InvalidArgument);
}
