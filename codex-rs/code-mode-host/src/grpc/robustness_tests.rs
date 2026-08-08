use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHost;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_protocol::ToolName;
use futures::FutureExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tonic::Code;
use tonic::Request;
use tonic::Status;
use uuid::Uuid;

use super::GrpcCodeModeHost;
use super::tests::execute_events;
use super::tests::execute_request;
use super::tests::open_session;
use super::tests::tool;
use super::validation::MAX_IDENTIFIER_BYTES;
use super::validation::MAX_TOOL_DESCRIPTION_BYTES;
use super::validation::MAX_TOOL_FILTERS;
use crate::MAX_ACTIVE_CELLS;
use crate::MAX_IN_FLIGHT_REQUESTS;
use crate::OUTGOING_CHANNEL_CAPACITY;

fn assert_invalid<T>(result: Result<T, Status>) {
    match result {
        Ok(_) => panic!("expected an oversized gRPC field to be rejected"),
        Err(error) => assert_eq!(error.code(), Code::InvalidArgument),
    }
}

fn invocation(cell_id: &str, name: &str) -> CodeModeNestedToolCall {
    CodeModeNestedToolCall {
        cell_id: CellId::new(cell_id.to_string()),
        runtime_tool_call_id: "runtime-call".to_string(),
        tool_name: ToolName::plain(name),
        tool_kind: CodeModeToolKind::Function,
        input: None,
    }
}

#[tokio::test]
async fn rejects_oversized_identifiers_tool_metadata_and_subscription_filters() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let oversized_id = "x".repeat(MAX_IDENTIFIER_BYTES + 1);

    assert_invalid(
        host.close_session(Request::new(proto::CloseSessionRequest {
            session_id: oversized_id.clone(),
        }))
        .await,
    );
    assert_invalid(
        host.close_session(Request::new(proto::CloseSessionRequest {
            session_id: "not-a-uuid".to_string(),
        }))
        .await,
    );
    assert_invalid(
        host.complete_tool_call(Request::new(proto::CompleteToolCallRequest {
            session_id: session_id.clone(),
            invocation_id: "not-a-uuid".to_string(),
            outcome: Some(proto::complete_tool_call_request::Outcome::Succeeded(
                proto::ToolCallSucceeded {
                    output_json: b"null".to_vec(),
                },
            )),
        }))
        .await,
    );
    assert_invalid(
        host.acknowledge_notification(Request::new(proto::AcknowledgeNotificationRequest {
            session_id: session_id.clone(),
            notification_id: "not-a-uuid".to_string(),
        }))
        .await,
    );
    assert_invalid(
        host.execute(Request::new(execute_request(
            &session_id,
            &oversized_id,
            "text(\"hello\");",
        )))
        .await,
    );
    assert_invalid(
        host.subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![proto::ToolName {
                name: oversized_id,
                namespace: None,
            }],
        }))
        .await,
    );
    assert_invalid(
        host.subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![
                proto::ToolName {
                    name: "echo".to_string(),
                    namespace: None,
                };
                MAX_TOOL_FILTERS + 1
            ],
        }))
        .await,
    );

    let mut request = execute_request(&session_id, "metadata", "text(\"hello\");");
    request.enabled_tools = vec![proto::ToolDefinition {
        description: "x".repeat(MAX_TOOL_DESCRIPTION_BYTES + 1),
        ..tool("echo")
    }];
    assert_invalid(host.execute(Request::new(request)).await);
    assert!(host.state.session(&session_id).is_ok());
}

#[tokio::test]
async fn dropping_an_unread_buffered_execution_outcome_retires_its_cell() {
    let host = GrpcCodeModeHost::new();
    let (session_id, mut events) = open_session(&host).await;
    let execution = host
        .execute(Request::new(execute_request(
            &session_id,
            "execution-abandoned",
            "await new Promise(() => {});",
        )))
        .await
        .expect("start execution")
        .into_inner();
    let _reserved_permits = (0..MAX_IN_FLIGHT_REQUESTS - 1)
        .map(|_| host.state.request_permit().expect("reserve request permit"))
        .collect::<Vec<_>>();
    let _execution_permit = tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        loop {
            if let Ok(permit) = host.state.request_permit() {
                return permit;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("execution outcome should be buffered before dropping its unread stream");

    drop(execution);

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 2), events.next())
            .await
            .expect("abandoned cell should close")
            .expect("cell closed event")
            .expect("session event"),
        proto::SessionEvent {
            event: Some(proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: "execution-abandoned".to_string(),
                cell_id: "1".to_string(),
                final_tool_call_sequence: 0,
            })),
        }
    );
}

#[tokio::test]
async fn closing_a_session_releases_buffered_cell_permits() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let session = host.state.session(&session_id).expect("open session");
    let mut permits = (0..MAX_ACTIVE_CELLS)
        .map(|_| {
            host.state
                .cell_permit()
                .expect("reserve active-cell permit")
        })
        .collect::<Vec<_>>();
    session
        .send_event_now(
            proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: "execution-queued".to_string(),
                cell_id: "1".to_string(),
                final_tool_call_sequence: 0,
            }),
            permits.pop(),
        )
        .expect("queue cell closure");

    host.close_session(Request::new(proto::CloseSessionRequest { session_id }))
        .await
        .expect("close session");

    assert!(
        host.state.cell_permit().is_ok(),
        "closing a session must release its queued active-cell permits"
    );
}

#[tokio::test]
async fn dropping_a_lease_after_its_host_shuts_down_closes_the_session() {
    let host = GrpcCodeModeHost::new();
    let (session_id, lease) = open_session(&host).await;
    let session = host.state.session(&session_id).expect("open session");

    drop(host);
    drop(lease);

    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), session.closed.cancelled())
        .await
        .expect("dropping a lease must close its session after the host disappears");
}

#[tokio::test]
async fn session_closure_cancels_pending_termination() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-termination",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.expect("execution outcome").unwrap();
    let session = host.state.session(&session_id).expect("open session");
    let termination = host.terminate(Request::new(proto::TerminateRequest {
        session_id,
        cell_id,
    }));
    tokio::pin!(termination);
    assert!(termination.as_mut().now_or_never().is_none());

    session.closed.cancel();
    let error = termination
        .await
        .expect_err("closed sessions must bound termination");

    assert_eq!(error.code(), Code::Cancelled);
}

#[tokio::test]
async fn oversized_encoded_tool_invocation_fails_without_closing_its_session() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let mut subscription = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .unwrap()
        .into_inner();
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-oversized",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.unwrap().unwrap();
    let session = host.state.session(&session_id).unwrap();
    let cancellation = CancellationToken::new();
    let (response, _receiver) = oneshot::channel();

    let error = session
        .dispatch_tool(
            invocation(&cell_id, "echo"),
            "execution-oversized".to_string(),
            Uuid::new_v4(),
            Some(vec![0; MAX_FRAME_BYTES]),
            response,
            &cancellation,
        )
        .await
        .unwrap_err();
    assert!(error.contains("gRPC message limit"));
    assert!(host.state.session(&session_id).is_ok());
    assert!(!session.closed.is_cancelled());

    let (response, _receiver) = oneshot::channel();
    let invocation_id = Uuid::new_v4();
    session
        .dispatch_tool(
            invocation(&cell_id, "echo"),
            "execution-oversized".to_string(),
            invocation_id,
            Some(b"{}".to_vec()),
            response,
            &cancellation,
        )
        .await
        .unwrap();
    let delivered = subscription.next().await.unwrap().unwrap();
    assert_eq!(delivered.invocation_id, invocation_id.to_string());
    assert_eq!(delivered.sequence, 1);
}

#[tokio::test]
async fn unmatched_tool_subscription_fails_without_closing_its_session() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let _unrelated = host
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
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-unmatched",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.unwrap().unwrap();
    let session = host.state.session(&session_id).unwrap();
    let cancellation = CancellationToken::new();
    let (response, _receiver) = oneshot::channel();

    assert!(
        session
            .dispatch_tool(
                invocation(&cell_id, "echo"),
                "execution-unmatched".to_string(),
                Uuid::new_v4(),
                Some(b"{}".to_vec()),
                response,
                &cancellation,
            )
            .await
            .is_err()
    );
    assert!(host.state.session(&session_id).is_ok());
    assert!(!session.closed.is_cancelled());

    let mut matching = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id,
            tool_names: vec![proto::ToolName {
                name: "echo".to_string(),
                namespace: None,
            }],
        }))
        .await
        .unwrap()
        .into_inner();
    let (response, _receiver) = oneshot::channel();
    let invocation_id = Uuid::new_v4();
    session
        .dispatch_tool(
            invocation(&cell_id, "echo"),
            "execution-unmatched".to_string(),
            invocation_id,
            Some(b"{}".to_vec()),
            response,
            &cancellation,
        )
        .await
        .unwrap();
    let delivered = matching.next().await.unwrap().unwrap();
    assert_eq!(delivered.invocation_id, invocation_id.to_string());
    assert_eq!(delivered.sequence, 1);
}

#[tokio::test]
async fn missing_selected_subscription_retries_another_matching_subscription() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let mut first = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .expect("subscribe first tool stream")
        .into_inner();
    let mut second = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .expect("subscribe second tool stream")
        .into_inner();
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-retry",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.expect("execution outcome").unwrap();
    let session = host.state.session(&session_id).expect("open session");
    let subscriptions = session
        .state
        .lock()
        .unwrap()
        .subscriptions
        .iter()
        .map(|subscription| (subscription.id, subscription.sender.clone()))
        .collect::<Vec<_>>();
    for (_, sender) in &subscriptions {
        for _ in 0..OUTGOING_CHANNEL_CAPACITY {
            sender
                .try_send(Ok(proto::ToolCall::default()))
                .expect("fill subscription queue");
        }
    }
    let cancellation = CancellationToken::new();
    let (response, _receiver) = oneshot::channel();
    let invocation_id = Uuid::new_v4();
    let dispatch = session.dispatch_tool(
        invocation(&cell_id, "echo"),
        "execution-retry".to_string(),
        invocation_id,
        /*input_json*/ None,
        response,
        &cancellation,
    );
    tokio::pin!(dispatch);
    assert!(dispatch.as_mut().now_or_never().is_none());
    session
        .state
        .lock()
        .unwrap()
        .subscriptions
        .retain(|subscription| subscription.id != subscriptions[0].0);

    first.next().await.expect("free first reservation").unwrap();
    assert!(dispatch.as_mut().now_or_never().is_none());
    second
        .next()
        .await
        .expect("free surviving reservation")
        .unwrap();
    dispatch.await.expect("retry surviving subscription");

    for _ in 1..OUTGOING_CHANNEL_CAPACITY {
        second.next().await.expect("drain buffered call").unwrap();
    }
    assert_eq!(
        second
            .next()
            .await
            .expect("retried invocation")
            .unwrap()
            .invocation_id,
        invocation_id.to_string()
    );
}

#[tokio::test]
async fn saturated_subscription_does_not_block_independently_filtered_tools() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let mut slow = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![proto::ToolName {
                name: "slow".to_string(),
                namespace: None,
            }],
        }))
        .await
        .unwrap()
        .into_inner();
    let mut fast = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: vec![proto::ToolName {
                name: "fast".to_string(),
                namespace: None,
            }],
        }))
        .await
        .unwrap()
        .into_inner();
    let (cell_id, mut execution) = execute_events(
        &host,
        execute_request(
            &session_id,
            "execution-backpressure",
            "await new Promise(() => {});",
        ),
    )
    .await;
    execution.next().await.unwrap().unwrap();
    let session = host.state.session(&session_id).unwrap();
    let cancellation = CancellationToken::new();
    let mut responses = Vec::new();

    for _ in 0..OUTGOING_CHANNEL_CAPACITY {
        let (response, receiver) = oneshot::channel();
        responses.push(receiver);
        session
            .dispatch_tool(
                invocation(&cell_id, "slow"),
                "execution-backpressure".to_string(),
                Uuid::new_v4(),
                /*input_json*/ None,
                response,
                &cancellation,
            )
            .await
            .unwrap();
    }

    let blocked_session = Arc::clone(&session);
    let blocked_cell = cell_id.clone();
    let blocked_cancellation = cancellation.clone();
    let (response, receiver) = oneshot::channel();
    responses.push(receiver);
    let blocked = tokio::spawn(async move {
        blocked_session
            .dispatch_tool(
                invocation(&blocked_cell, "slow"),
                "execution-backpressure".to_string(),
                Uuid::new_v4(),
                /*input_json*/ None,
                response,
                &blocked_cancellation,
            )
            .await
    });
    tokio::task::yield_now().await;
    assert!(!blocked.is_finished());

    let (response, receiver) = oneshot::channel();
    responses.push(receiver);
    let invocation_id = Uuid::new_v4();
    tokio::time::timeout(
        Duration::from_secs(1),
        session.dispatch_tool(
            invocation(&cell_id, "fast"),
            "execution-backpressure".to_string(),
            invocation_id,
            /*input_json*/ None,
            response,
            &cancellation,
        ),
    )
    .await
    .expect("saturated subscription must not block another tool")
    .unwrap();
    assert_eq!(
        fast.next().await.unwrap().unwrap().invocation_id,
        invocation_id.to_string()
    );

    slow.next().await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(1), blocked)
        .await
        .expect("draining the subscription should release its blocked invocation")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn dropping_subscriptions_only_retires_sessions_with_unread_calls() {
    let host = GrpcCodeModeHost::new();
    let (session_id, _events) = open_session(&host).await;
    let idle = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .unwrap()
        .into_inner();
    drop(idle);

    let session = host.state.session(&session_id).unwrap();
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        while !session.state.lock().unwrap().subscriptions.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("idle subscription should be removed without retiring its session");
    assert!(host.state.session(&session_id).is_ok());

    let first = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .unwrap()
        .into_inner();
    let mut second = host
        .subscribe_to_tool_calls(Request::new(proto::SubscribeToToolCallsRequest {
            session_id: session_id.clone(),
            tool_names: Vec::new(),
        }))
        .await
        .unwrap()
        .into_inner();
    let mut request = execute_request(
        &session_id,
        "execution-subscription-drop",
        r#"await tools.echo({attempt: 1});"#,
    );
    request.yield_time_ms = Some(/*value*/ 10_000);
    request.enabled_tools = vec![tool("echo")];
    let (_cell_id, mut execution) = execute_events(&host, request).await;
    let first_subscription_id = session.state.lock().unwrap().subscriptions[0].id;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        loop {
            let owned = session
                .state
                .lock()
                .unwrap()
                .pending_invocations
                .values()
                .any(|invocation| invocation.subscription_id == first_subscription_id);
            if owned {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first subscription should own an unread tool call");
    drop(first);

    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        while host.state.session(&session_id).is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("losing an unread call must retire its lease without leaving a sequence gap");
    assert!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 2), second.next())
            .await
            .expect("other subscriptions should close with their lease")
            .is_none()
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 2), execution.next())
            .await
            .expect("execution should retire with its lease")
            .is_none()
    );
}
