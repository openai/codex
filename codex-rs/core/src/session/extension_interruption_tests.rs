//! Regression coverage for cancellation while submitting a conditional interrupt.

use super::*;
use crate::session::SessionIo;
use crate::session::completed_session_loop_termination;
use crate::session::tests::HeldStepTask;
use crate::session::tests::make_session_and_context_with_rx;
use crate::state::TaskKind;
use crate::thread_startup_metadata::ThreadStartupMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::watch;

#[tokio::test]
async fn interrupt_if_no_pending_input_handles_cancelled_submission() {
    let (session, turn_context, rx_event) = make_session_and_context_with_rx().await;
    session
        .spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(Notify::new()),
            },
        )
        .await;
    let cancellation_token = session
        .active_turn
        .lock()
        .await
        .as_ref()
        .expect("active turn")
        .task
        .as_ref()
        .expect("active task")
        .cancellation_token
        .clone();
    let (tx_sub, rx_sub) = async_channel::bounded(/*cap*/ 1);
    let thread = CodexThread::new(
        Arc::clone(&session),
        SessionIo {
            tx_sub,
            rx_event,
            agent_status: watch::channel(AgentStatus::PendingInit).1,
            session_loop_termination: completed_session_loop_termination(),
        },
        ThreadStartupMetadata::from(&SessionConfiguredEvent {
            session_id: session.session_id(),
            thread_id: session.thread_id(),
            forked_from_id: None,
            parent_thread_id: None,
            thread_source: None,
            thread_name: None,
            model: "test".to_string(),
            model_provider_id: "test".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            permission_profile: turn_context.permission_profile(),
            active_permission_profile: None,
            cwd: turn_context.config.cwd.clone(),
            reasoning_effort: None,
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
        /*rollout_path*/ None,
        SessionSource::Cli,
    );
    thread.submit(Op::Interrupt).await.expect("fill the queue");

    let interrupt = thread.interrupt_if_no_pending_input(&turn_context.sub_id);
    tokio::pin!(interrupt);
    assert!(futures::poll!(interrupt.as_mut()).is_pending());
    // The pending submission still owns its reply sender when cancellation wins.
    cancellation_token.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), interrupt)
            .await
            .expect("cancellation should unblock the submission")
            .expect("cancelling a submission does not mean the agent died"),
        false,
    );
    assert!(!rx_sub.is_closed());
    assert_eq!(rx_sub.len(), 1);
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
}
