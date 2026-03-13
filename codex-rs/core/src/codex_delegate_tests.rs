use super::*;
use async_channel::bounded;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio::time::timeout;

#[tokio::test]
async fn forward_events_cancelled_while_send_blocked_shuts_down_delegate() {
    let (tx_events, rx_events) = bounded(1);
    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let (session, ctx, _rx_evt) = crate::codex::make_session_and_context_with_rx().await;
    let codex = Arc::new(Codex {
        tx_sub,
        rx_event: rx_events,
        agent_status,
        session: Arc::clone(&session),
        session_loop_termination: completed_session_loop_termination(),
    });

    let (tx_out, rx_out) = bounded(1);
    tx_out
        .send(Event {
            id: "full".to_string(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-1".to_string()),
                reason: TurnAbortReason::Interrupted,
            }),
        })
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    let forward = tokio::spawn(forward_events(
        Arc::clone(&codex),
        tx_out.clone(),
        session,
        ctx,
        Arc::new(Mutex::new(HashMap::new())),
        cancel.clone(),
    ));

    tx_events
        .send(Event {
            id: "evt".to_string(),
            msg: EventMsg::RawResponseItem(RawResponseItemEvent {
                item: ResponseItem::CustomToolCall {
                    id: None,
                    status: None,
                    call_id: "call-1".to_string(),
                    name: "tool".to_string(),
                    input: "{}".to_string(),
                },
            }),
        })
        .await
        .unwrap();

    drop(tx_events);
    cancel.cancel();
    timeout(std::time::Duration::from_millis(1000), forward)
        .await
        .expect("forward_events hung")
        .expect("forward_events join error");

    let received = rx_out.recv().await.expect("prefilled event missing");
    assert_eq!("full", received.id);
    let mut ops = Vec::new();
    while let Ok(sub) = rx_sub.try_recv() {
        ops.push(sub.op);
    }
    assert!(
        ops.iter().any(|op| matches!(op, Op::Interrupt)),
        "expected Interrupt op after cancellation"
    );
    assert!(
        ops.iter().any(|op| matches!(op, Op::Shutdown)),
        "expected Shutdown op after cancellation"
    );
}

#[tokio::test]
async fn forward_ops_preserves_submission_trace_context() {
    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_tx_events, rx_events) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let (session, _ctx, _rx_evt) = crate::codex::make_session_and_context_with_rx().await;
    let codex = Arc::new(Codex {
        tx_sub,
        rx_event: rx_events,
        agent_status,
        session,
        session_loop_termination: completed_session_loop_termination(),
    });
    let (tx_ops, rx_ops) = bounded(1);
    let cancel = CancellationToken::new();
    let forward = tokio::spawn(forward_ops(Arc::clone(&codex), rx_ops, cancel));

    let submission = Submission {
        id: "sub-1".to_string(),
        op: Op::Interrupt,
        trace: Some(codex_protocol::protocol::W3cTraceContext {
            traceparent: Some(
                "00-1234567890abcdef1234567890abcdef-1234567890abcdef-01".to_string(),
            ),
            tracestate: Some("vendor=state".to_string()),
        }),
    };
    tx_ops.send(submission.clone()).await.unwrap();
    drop(tx_ops);

    let forwarded = timeout(Duration::from_secs(1), rx_sub.recv())
        .await
        .expect("forward_ops hung")
        .expect("forwarded submission missing");
    assert_eq!(submission.id, forwarded.id);
    assert_eq!(submission.op, forwarded.op);
    assert_eq!(submission.trace, forwarded.trace);

    timeout(Duration::from_secs(1), forward)
        .await
        .expect("forward_ops did not exit")
        .expect("forward_ops join error");
}

#[tokio::test]
async fn handle_request_permissions_uses_tool_call_id_for_round_trip() {
    let (parent_session, parent_ctx, rx_events) =
        crate::codex::make_session_and_context_with_rx().await;
    *parent_session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_tx_events, rx_events_child) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let codex = Arc::new(Codex {
        tx_sub,
        rx_event: rx_events_child,
        agent_status,
        session: Arc::clone(&parent_session),
        session_loop_termination: completed_session_loop_termination(),
    });

    let call_id = "tool-call-1".to_string();
    let expected_response = RequestPermissionsResponse {
        permissions: RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            ..RequestPermissionProfile::default()
        },
        scope: PermissionGrantScope::Turn,
    };
    let cancel_token = CancellationToken::new();
    let request_call_id = call_id.clone();

    let handle = tokio::spawn({
        let codex = Arc::clone(&codex);
        let parent_session = Arc::clone(&parent_session);
        let parent_ctx = Arc::clone(&parent_ctx);
        let cancel_token = cancel_token.clone();
        async move {
            handle_request_permissions(
                codex.as_ref(),
                &parent_session,
                &parent_ctx,
                RequestPermissionsEvent {
                    call_id: request_call_id,
                    turn_id: "child-turn-1".to_string(),
                    reason: Some("need access".to_string()),
                    permissions: RequestPermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        ..RequestPermissionProfile::default()
                    },
                },
                &cancel_token,
            )
            .await;
        }
    });

    let request_event = timeout(Duration::from_secs(1), rx_events.recv())
        .await
        .expect("request_permissions event timed out")
        .expect("request_permissions event missing");
    let EventMsg::RequestPermissions(request) = request_event.msg else {
        panic!("expected RequestPermissions event");
    };
    assert_eq!(request.call_id, call_id.clone());

    parent_session
        .notify_request_permissions_response(&call_id, expected_response.clone())
        .await;

    timeout(Duration::from_secs(1), handle)
        .await
        .expect("handle_request_permissions hung")
        .expect("handle_request_permissions join error");

    let submission = timeout(Duration::from_secs(1), rx_sub.recv())
        .await
        .expect("request_permissions response timed out")
        .expect("request_permissions response missing");
    assert_eq!(
        submission.op,
        Op::RequestPermissionsResponse {
            id: call_id,
            response: expected_response,
        }
    );
}

#[tokio::test]
async fn handle_exec_approval_uses_call_id_for_guardian_review_and_approval_id_for_reply() {
    let (parent_session, parent_ctx, rx_events) =
        crate::codex::make_session_and_context_with_rx().await;
    let mut parent_ctx = Arc::try_unwrap(parent_ctx).expect("single turn context ref");
    let mut config = (*parent_ctx.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    parent_ctx.config = Arc::new(config);
    parent_ctx
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set on-request policy");
    let parent_ctx = Arc::new(parent_ctx);

    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_tx_events, rx_events_child) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let codex = Arc::new(Codex {
        tx_sub,
        rx_event: rx_events_child,
        agent_status,
        session: Arc::clone(&parent_session),
        session_loop_termination: completed_session_loop_termination(),
    });

    let cancel_token = CancellationToken::new();
    let handle = tokio::spawn({
        let codex = Arc::clone(&codex);
        let parent_session = Arc::clone(&parent_session);
        let parent_ctx = Arc::clone(&parent_ctx);
        let cancel_token = cancel_token.clone();
        async move {
            handle_exec_approval(
                codex.as_ref(),
                "child-turn-1".to_string(),
                &parent_session,
                &parent_ctx,
                ExecApprovalRequestEvent {
                    call_id: "command-item-1".to_string(),
                    approval_id: Some("callback-approval-1".to_string()),
                    turn_id: "child-turn-1".to_string(),
                    command: vec!["rm".to_string(), "-rf".to_string(), "tmp".to_string()],
                    cwd: PathBuf::from("/tmp"),
                    reason: Some("unsafe subcommand".to_string()),
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
                    skill_metadata: None,
                    available_decisions: Some(vec![
                        ReviewDecision::Approved,
                        ReviewDecision::Abort,
                    ]),
                    parsed_cmd: Vec::new(),
                },
                &cancel_token,
            )
            .await;
        }
    });

    let assessment_event = timeout(Duration::from_secs(2), async {
        loop {
            let event = rx_events.recv().await.expect("guardian assessment event");
            if let EventMsg::GuardianAssessment(assessment) = event.msg {
                return assessment;
            }
        }
    })
    .await
    .expect("timed out waiting for guardian assessment");
    assert_eq!(
        assessment_event,
        GuardianAssessmentEvent {
            id: "command-item-1".to_string(),
            turn_id: parent_ctx.sub_id.clone(),
            status: GuardianAssessmentStatus::InProgress,
            risk_score: None,
            risk_level: None,
            rationale: None,
            action: Some(json!({
                "tool": "shell",
                "command": "rm -rf tmp",
                "cwd": "/tmp",
            })),
        }
    );

    cancel_token.cancel();

    timeout(Duration::from_secs(2), handle)
        .await
        .expect("handle_exec_approval hung")
        .expect("handle_exec_approval join error");

    let submission = timeout(Duration::from_secs(2), rx_sub.recv())
        .await
        .expect("exec approval response timed out")
        .expect("exec approval response missing");
    assert_eq!(
        submission.op,
        Op::ExecApproval {
            id: "callback-approval-1".to_string(),
            turn_id: Some("child-turn-1".to_string()),
            decision: ReviewDecision::Abort,
        }
    );
}
