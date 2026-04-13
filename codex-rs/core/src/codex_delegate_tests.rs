use super::*;
use crate::mcp_tool_call::MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC;
use crate::mcp_tool_call::MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX;
use async_channel::bounded;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use core_test_support::PathBufExt;
use core_test_support::test_path_buf;
use core_test_support::tracing::install_test_tracing;
use opentelemetry::trace::TraceContextExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Instrument;

fn test_trace_context(trace_id: &str, span_id: &str) -> W3cTraceContext {
    W3cTraceContext {
        traceparent: Some(format!("00-{trace_id}-{span_id}-01")),
        tracestate: Some("vendor=value".to_string()),
    }
}

#[tokio::test]
async fn forward_events_cancelled_while_send_blocked_shuts_down_delegate() {
    let (tx_events, rx_events) = bounded(1);
    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let (session, ctx, _rx_evt) = crate::session::tests::make_session_and_context_with_rx().await;
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
                completed_at: None,
                duration_ms: None,
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
    let (session, _ctx, _rx_evt) = crate::session::tests::make_session_and_context_with_rx().await;
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
        crate::session::tests::make_session_and_context_with_rx().await;
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
    let delegated_cwd = parent_ctx.cwd.join("delegated-cwd");
    let cancel_token = CancellationToken::new();
    let request_call_id = call_id.clone();
    let request_cwd = delegated_cwd.clone();

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
                    cwd: Some(request_cwd),
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
    assert_eq!(request.cwd, Some(delegated_cwd));

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
        crate::session::tests::make_session_and_context_with_rx().await;
    let mut parent_ctx = Arc::try_unwrap(parent_ctx).expect("single turn context ref");
    let parent_trace = test_trace_context("00000000000000000000000000000011", "0000000000000022");
    let mut config = (*parent_ctx.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    parent_ctx.config = Arc::new(config);
    parent_ctx
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set on-request policy");
    parent_ctx.trace_id = Some("00000000000000000000000000000011".to_string());
    parent_ctx.trace_context = Some(parent_trace.clone());
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
                    cwd: test_path_buf("/tmp").abs(),
                    reason: Some("unsafe subcommand".to_string()),
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
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
    let expected_action = GuardianAssessmentAction::Command {
        source: GuardianCommandSource::Shell,
        command: "rm -rf tmp".to_string(),
        cwd: test_path_buf("/tmp").abs(),
    };
    assert!(!assessment_event.id.is_empty());
    assert_eq!(
        assessment_event.target_item_id.as_deref(),
        Some("command-item-1")
    );
    assert_eq!(assessment_event.turn_id, parent_ctx.sub_id);
    assert_eq!(
        assessment_event.status,
        GuardianAssessmentStatus::InProgress
    );
    assert_eq!(assessment_event.risk_level, None);
    assert_eq!(assessment_event.user_authorization, None);
    assert_eq!(assessment_event.rationale, None);
    assert_eq!(assessment_event.decision_source, None);
    assert_eq!(assessment_event.action, expected_action);

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
    assert_eq!(submission.trace, Some(parent_trace));
}

#[tokio::test]
async fn delegated_mcp_guardian_abort_returns_synthetic_decline_answer() {
    let (parent_session, parent_ctx, _rx_events) =
        crate::session::tests::make_session_and_context_with_rx().await;
    let mut parent_ctx = Arc::try_unwrap(parent_ctx).expect("single turn context ref");
    let mut config = (*parent_ctx.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    parent_ctx.config = Arc::new(config);
    parent_ctx
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set on-request policy");
    let parent_ctx = Arc::new(parent_ctx);

    let pending_mcp_invocations = Arc::new(Mutex::new(HashMap::from([(
        "call-1".to_string(),
        McpInvocation {
            server: "custom_server".to_string(),
            tool: "dangerous_tool".to_string(),
            arguments: None,
        },
    )])));
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let response = maybe_auto_review_mcp_request_user_input(
        &parent_session,
        &parent_ctx,
        &pending_mcp_invocations,
        &RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: "child-turn-1".to_string(),
            questions: vec![RequestUserInputQuestion {
                id: format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_call-1"),
                header: "Approve app tool call?".to_string(),
                question: "Allow this app tool?".to_string(),
                is_other: false,
                is_secret: false,
                options: None,
            }],
        },
        &cancel_token,
    )
    .await;

    assert_eq!(
        response,
        Some(RequestUserInputResponse {
            answers: HashMap::from([(
                format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_call-1"),
                RequestUserInputAnswer {
                    answers: vec![MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC.to_string()],
                },
            )]),
        })
    );
}

#[tokio::test]
async fn handle_request_user_input_guardian_submission_uses_parent_trace() {
    let (parent_session, parent_ctx, _rx_events) =
        crate::codex::make_session_and_context_with_rx().await;
    let mut parent_ctx = Arc::try_unwrap(parent_ctx).expect("single turn context ref");
    let parent_trace = test_trace_context("00000000000000000000000000000033", "0000000000000044");
    let mut config = (*parent_ctx.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    parent_ctx.config = Arc::new(config);
    parent_ctx
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set on-request policy");
    parent_ctx.trace_id = Some("00000000000000000000000000000033".to_string());
    parent_ctx.trace_context = Some(parent_trace.clone());
    let parent_ctx = Arc::new(parent_ctx);

    let pending_mcp_invocations = Arc::new(Mutex::new(HashMap::from([(
        "call-1".to_string(),
        McpInvocation {
            server: "custom_server".to_string(),
            tool: "dangerous_tool".to_string(),
            arguments: None,
        },
    )])));
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
    cancel_token.cancel();

    handle_request_user_input(
        codex.as_ref(),
        "user-input-1".to_string(),
        &parent_session,
        &parent_ctx,
        &pending_mcp_invocations,
        RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: "child-turn-1".to_string(),
            questions: vec![RequestUserInputQuestion {
                id: format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_call-1"),
                header: "Approve app tool call?".to_string(),
                question: "Allow this app tool?".to_string(),
                is_other: false,
                is_secret: false,
                options: None,
            }],
        },
        &cancel_token,
    )
    .await;

    let submission = timeout(Duration::from_secs(2), rx_sub.recv())
        .await
        .expect("user input response timed out")
        .expect("user input response missing");
    assert_eq!(
        submission.op,
        Op::UserInputAnswer {
            id: "user-input-1".to_string(),
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_call-1"),
                    RequestUserInputAnswer {
                        answers: vec![MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC.to_string()],
                    },
                )]),
            },
        }
    );
    assert_eq!(submission.trace, Some(parent_trace));
}

#[tokio::test]
async fn handle_patch_approval_guardian_submission_uses_parent_trace() {
    let (parent_session, parent_ctx, _rx_events) =
        crate::codex::make_session_and_context_with_rx().await;
    let mut parent_ctx = Arc::try_unwrap(parent_ctx).expect("single turn context ref");
    let parent_trace = test_trace_context("00000000000000000000000000000077", "0000000000000088");
    let mut config = (*parent_ctx.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    parent_ctx.config = Arc::new(config);
    parent_ctx
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set on-request policy");
    parent_ctx.trace_id = Some("00000000000000000000000000000077".to_string());
    parent_ctx.trace_context = Some(parent_trace.clone());
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
    cancel_token.cancel();

    handle_patch_approval(
        codex.as_ref(),
        "patch-approval-event".to_string(),
        &parent_session,
        &parent_ctx,
        ApplyPatchApprovalRequestEvent {
            call_id: "patch-call-1".to_string(),
            turn_id: "child-turn-1".to_string(),
            changes: HashMap::from([(
                PathBuf::from("src/main.rs"),
                codex_protocol::protocol::FileChange::Update {
                    unified_diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
                    move_path: None,
                },
            )]),
            reason: Some("dangerous patch".to_string()),
            grant_root: None,
        },
        &cancel_token,
    )
    .await;

    let submission = timeout(Duration::from_secs(2), rx_sub.recv())
        .await
        .expect("patch approval response timed out")
        .expect("patch approval response missing");
    assert_eq!(
        submission.op,
        Op::PatchApproval {
            id: "patch-call-1".to_string(),
            decision: ReviewDecision::Abort,
        }
    );
    assert_eq!(submission.trace, Some(parent_trace));
}

#[tokio::test]
async fn guardian_review_span_uses_parent_turn_trace_context() {
    let _trace_test_context = install_test_tracing("codex-core-tests");
    let (_session, mut turn) = crate::codex::make_session_and_context().await;
    let parent_trace = test_trace_context("00000000000000000000000000000011", "0000000000000022");
    turn.trace_id = Some("00000000000000000000000000000011".to_string());
    turn.trace_context = Some(parent_trace.clone());
    let trace = crate::guardian::guardian_review_trace_for_test(Arc::new(turn), None)
        .await
        .expect("guardian review trace");
    let actual =
        codex_otel::context_from_w3c_trace_context(&trace).expect("actual guardian review trace");
    let expected =
        codex_otel::context_from_w3c_trace_context(&parent_trace).expect("expected parent trace");
    assert_eq!(
        actual.span().span_context().trace_id(),
        expected.span().span_context().trace_id()
    );
    assert_ne!(
        actual.span().span_context().span_id(),
        expected.span().span_context().span_id()
    );
}

#[tokio::test]
async fn guardian_review_submit_trace_prefers_current_review_span_and_falls_back_to_turn_trace() {
    let _trace_test_context = install_test_tracing("codex-core-tests");
    let (_session, mut turn) = crate::codex::make_session_and_context().await;
    let fallback_trace = test_trace_context("00000000000000000000000000000033", "0000000000000044");
    let current_review_trace =
        test_trace_context("00000000000000000000000000000055", "0000000000000066");
    turn.trace_id = Some("00000000000000000000000000000033".to_string());
    turn.trace_context = Some(fallback_trace.clone());
    let turn = Arc::new(turn);

    let current_span_trace = async {
        crate::guardian::guardian_review_submit_trace_for_test(turn.as_ref())
            .expect("current review trace")
    }
    .instrument({
        let span = tracing::info_span!("guardian_review_submit");
        assert!(codex_otel::set_parent_from_w3c_trace_context(
            &span,
            &current_review_trace
        ));
        span
    })
    .await;
    let current_actual = codex_otel::context_from_w3c_trace_context(&current_span_trace)
        .expect("current span trace context");
    let current_expected = codex_otel::context_from_w3c_trace_context(&current_review_trace)
        .expect("expected current review trace");
    assert_eq!(
        current_actual.span().span_context().trace_id(),
        current_expected.span().span_context().trace_id()
    );

    let fallback_actual = crate::guardian::guardian_review_submit_trace_for_test(turn.as_ref())
        .expect("fallback turn trace");
    let fallback_actual = codex_otel::context_from_w3c_trace_context(&fallback_actual)
        .expect("fallback trace context");
    let fallback_expected = codex_otel::context_from_w3c_trace_context(&fallback_trace)
        .expect("expected fallback trace");
    assert_eq!(
        fallback_actual.span().span_context().trace_id(),
        fallback_expected.span().span_context().trace_id()
    );
}
