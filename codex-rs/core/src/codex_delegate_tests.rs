use super::*;
use async_channel::bounded;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::watch;
use tokio::time::timeout;

struct ThreadStartRecorder(Arc<AtomicUsize>);

impl ThreadLifecycleContributor<Config> for ThreadStartRecorder {
    fn on_thread_start<'a>(
        &'a self,
        _input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(()))
    }
}

#[tokio::test]
async fn forward_events_filters_private_events_before_blocked_send_is_cancelled() {
    let (tx_events, rx_events) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (tx_sub, rx_sub) = bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (_agent_status_tx, agent_status) = watch::channel(AgentStatus::PendingInit);
    let io = Arc::new(SessionIo {
        tx_sub,
        rx_event: rx_events,
        agent_status,
        session_loop_termination: completed_session_loop_termination(),
    });

    let (tx_out, rx_out) = bounded(1);
    tx_out
        .send(Event {
            id: "full".to_string(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-1".to_string()),
                started_at: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            }),
        })
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    let forward = tokio::spawn(forward_events(
        Arc::clone(&io),
        tx_out.clone(),
        cancel.clone(),
    ));

    for msg in [
        EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
            server: "pending".to_string(),
            status: McpStartupStatus::Starting,
        }),
        EventMsg::McpStartupComplete(McpStartupCompleteEvent::default()),
    ] {
        tx_events
            .send(Event {
                id: "delegate-startup".to_string(),
                msg,
            })
            .await
            .unwrap();
    }
    let visible_msg = EventMsg::RawResponseItem(RawResponseItemEvent {
        item: ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "tool".to_string(),
            namespace: None,
            input: "{}".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
    });
    for id in ["visible-1", "visible-2", "blocked"] {
        tx_events
            .send(Event {
                id: id.to_string(),
                msg: visible_msg.clone(),
            })
            .await
            .unwrap();
    }

    drop(tx_events);
    let received = rx_out.recv().await.expect("prefilled event missing");
    assert_eq!(received.id, "full");
    let received = rx_out.recv().await.expect("visible event missing");
    assert_eq!(received.id, "visible-1");
    cancel.cancel();
    timeout(std::time::Duration::from_millis(1000), forward)
        .await
        .expect("forward_events hung")
        .expect("forward_events join error");

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
    let io = Arc::new(SessionIo {
        tx_sub,
        rx_event: rx_events,
        agent_status,
        session_loop_termination: completed_session_loop_termination(),
    });
    let (tx_ops, rx_ops) = bounded(1);
    let cancel = CancellationToken::new();
    let forward = tokio::spawn(forward_ops(Arc::clone(&io), rx_ops, cancel));

    let submission = Submission {
        id: "sub-1".to_string(),
        op: Op::Interrupt,
        trace: Some(codex_protocol::protocol::W3cTraceContext {
            traceparent: Some(
                "00-1234567890abcdef1234567890abcdef-1234567890abcdef-01".to_string(),
            ),
            tracestate: Some("vendor=state".to_string()),
        }),
        parent_turn_id: Some("parent-turn".to_string()),
        root_turn_id: Some("root-turn".to_string()),
    };
    tx_ops.send(submission).await.unwrap();
    drop(tx_ops);

    let forwarded = timeout(Duration::from_secs(1), rx_sub.recv())
        .await
        .expect("forward_ops hung")
        .expect("forwarded submission missing");
    assert_eq!("sub-1", forwarded.id);
    assert!(matches!(forwarded.op, Op::Interrupt));
    assert_eq!(
        forwarded.trace,
        Some(codex_protocol::protocol::W3cTraceContext {
            traceparent: Some(
                "00-1234567890abcdef1234567890abcdef-1234567890abcdef-01".to_string(),
            ),
            tracestate: Some("vendor=state".to_string()),
        })
    );
    assert_eq!(Some("parent-turn".to_string()), forwarded.parent_turn_id);

    timeout(Duration::from_secs(1), forward)
        .await
        .expect("forward_ops did not exit")
        .expect("forward_ops join error");
}

#[tokio::test]
async fn run_codex_thread_interactive_respects_pre_cancelled_spawn() {
    let (parent_session, parent_ctx, _rx_events) =
        crate::session::tests::make_session_and_context_with_rx().await;
    let mut config = parent_ctx.config.as_ref().clone();
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();
    let parent_environments = parent_ctx.environments.clone();

    let result = timeout(
        Duration::from_secs(/*secs*/ 1),
        run_codex_thread_interactive(
            config,
            Arc::clone(&parent_session.services.auth_manager),
            Arc::clone(&parent_session.services.models_manager),
            parent_session,
            parent_ctx,
            parent_environments,
            cancel_token,
            SubAgentSource::Review,
            /*initial_history*/ None,
            crate::session::GitEnrichmentPolicy::Fresh,
            codex_sandboxing::WindowsSandboxProxySettingsMode::Reconcile,
        ),
    )
    .await
    .expect("cancelled delegate spawn should not hang");

    assert!(matches!(
        result,
        Err(err) if matches!(err.details(), CodexErrorDetails::TurnAborted)
    ));
}

#[tokio::test]
async fn guardian_delegates_do_not_inherit_parent_extensions() {
    let (mut parent_session, parent_ctx, _rx_events) =
        crate::session::tests::make_session_and_context_with_rx().await;
    let thread_starts = Arc::new(AtomicUsize::new(0));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions
        .thread_lifecycle_contributor(Arc::new(ThreadStartRecorder(Arc::clone(&thread_starts))));
    Arc::get_mut(&mut parent_session)
        .expect("parent session should be uniquely owned")
        .services
        .extensions = Arc::new(extensions.build());

    for (subagent_source, expected_thread_starts, expected_thread_source) in [
        (
            SubAgentSource::Other(crate::guardian::GUARDIAN_REVIEWER_NAME.to_string()),
            0,
            ThreadSource::GuardianReview,
        ),
        (SubAgentSource::Review, 1, ThreadSource::Subagent),
    ] {
        let mut config = parent_ctx.config.as_ref().clone();
        config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
        let (session, io) = run_codex_thread_interactive(
            config,
            Arc::clone(&parent_session.services.auth_manager),
            Arc::clone(&parent_session.services.models_manager),
            Arc::clone(&parent_session),
            Arc::clone(&parent_ctx),
            parent_ctx.environments.clone(),
            CancellationToken::new(),
            subagent_source,
            /*initial_history*/ None,
            crate::session::GitEnrichmentPolicy::Fresh,
            codex_sandboxing::WindowsSandboxProxySettingsMode::Reconcile,
        )
        .await
        .expect("delegate session should start");

        assert_eq!(
            session
                .services
                .extensions
                .thread_lifecycle_contributors()
                .len(),
            expected_thread_starts
        );
        assert_eq!(thread_starts.load(Ordering::SeqCst), expected_thread_starts);
        assert_eq!(
            session.thread_config_snapshot().await.thread_source,
            Some(expected_thread_source)
        );
        io.shutdown_and_wait()
            .await
            .expect("delegate session should shut down");
    }
}

#[tokio::test]
async fn run_codex_thread_interactive_rejects_approval_policy_that_can_prompt() {
    let (parent_session, parent_ctx, _rx_events) =
        crate::session::tests::make_session_and_context_with_rx().await;
    let mut config = parent_ctx.config.as_ref().clone();
    config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let parent_environments = parent_ctx.environments.clone();

    let result = run_codex_thread_interactive(
        config,
        Arc::clone(&parent_session.services.auth_manager),
        Arc::clone(&parent_session.services.models_manager),
        parent_session,
        parent_ctx,
        parent_environments,
        CancellationToken::new(),
        SubAgentSource::Review,
        /*initial_history*/ None,
        crate::session::GitEnrichmentPolicy::Fresh,
        codex_sandboxing::WindowsSandboxProxySettingsMode::Reconcile,
    )
    .await;

    assert!(matches!(
        result,
        Err(err)
            if matches!(
                err.details(),
                CodexErrorDetails::InvalidRequest(message)
                    if message == "Codex delegates require approval policy `never`"
            )
    ));
}
