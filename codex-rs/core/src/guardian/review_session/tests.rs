//! Focused guardian review-session tests that need private module access.
//!
//! The broader guardian behavior tests stay in `guardian/tests.rs`. This file is reserved for the
//! lower-level session-manager invariants that are hard to exercise cleanly through the public API.

use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::execution::run_before_review_deadline_with_cancel;
use super::*;
use crate::agent::AgentStatus;
use crate::codex::session_loop_termination_from_handle;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::Submission;

async fn guardian_review_session_with_shutdown_signal() -> (
    Arc<GuardianReviewSession>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let (child_session, _child_turn_context) = crate::codex::make_session_and_context().await;
    let child_session = Arc::new(child_session);
    let child_config = child_session.get_config().await;
    let (child_tx_sub, child_rx_sub) = async_channel::bounded(4);
    let (_child_tx_event, child_rx_event) = async_channel::unbounded();
    let (_child_status_tx, child_agent_status) = watch::channel(AgentStatus::PendingInit);
    let (child_shutdown_tx, child_shutdown_rx) = tokio::sync::oneshot::channel();
    let child_session_loop_handle = tokio::spawn(async move {
        let shutdown: Submission = child_rx_sub
            .recv()
            .await
            .expect("child shutdown submission");
        assert_eq!(shutdown.op, Op::Shutdown);
        child_shutdown_tx
            .send(())
            .expect("child shutdown signal should be delivered");
    });
    let child_codex = Codex {
        tx_sub: child_tx_sub,
        rx_event: child_rx_event,
        agent_status: child_agent_status,
        session: child_session,
        session_loop_termination: session_loop_termination_from_handle(child_session_loop_handle),
    };
    let review_session = Arc::new(GuardianReviewSession::new(
        child_codex,
        CancellationToken::new(),
        child_config.as_ref().clone(),
        /*has_prior_review*/ false,
    ));

    (review_session, child_shutdown_rx)
}

#[test]
fn guardian_review_session_config_change_changes_spawn_config() {
    let parent_config = crate::config::test_config();
    let cached_spawn_config =
        build_guardian_review_session_config(&parent_config, None, "active-model", None)
            .expect("cached guardian config");

    let mut changed_parent_config = parent_config;
    changed_parent_config.model_provider.base_url =
        Some("https://guardian.example.invalid/v1".to_string());
    let next_spawn_config =
        build_guardian_review_session_config(&changed_parent_config, None, "active-model", None)
            .expect("next guardian config");

    assert_ne!(cached_spawn_config, next_spawn_config);
}

#[test]
fn guardian_review_session_config_disables_reasoning_summary_and_personality() {
    let mut parent_config = crate::config::test_config();
    parent_config.model_reasoning_summary =
        Some(codex_protocol::config_types::ReasoningSummary::Detailed);
    parent_config.personality = Some(codex_protocol::config_types::Personality::Pragmatic);

    let guardian_config =
        build_guardian_review_session_config(&parent_config, None, "active-model", None)
            .expect("guardian config");

    assert_eq!(
        guardian_config.model_reasoning_summary,
        Some(codex_protocol::config_types::ReasoningSummary::None)
    );
    assert_eq!(guardian_config.personality, None);
}

#[tokio::test]
async fn install_spawned_trunk_waits_for_shutdown_when_manager_is_shutting_down() {
    let manager = GuardianReviewSessionManager::default();
    manager.state.lock().await.shutdown_started = true;

    let (review_session, child_shutdown_rx) = guardian_review_session_with_shutdown_signal().await;

    let trunk = manager.install_spawned_trunk(review_session).await;

    assert!(trunk.is_none());
    tokio::time::timeout(Duration::from_millis(10), child_shutdown_rx)
        .await
        .expect("install_spawned_trunk should wait for guardian shutdown")
        .expect("guardian shutdown signal");
}

#[tokio::test]
async fn eager_trunk_init_does_not_replace_existing_trunk() {
    let manager = GuardianReviewSessionManager::default();
    let (trunk_session, _child_shutdown_rx) = guardian_review_session_with_shutdown_signal().await;
    manager.state.lock().await.trunk = Some(Arc::clone(&trunk_session));

    let (parent_session, parent_turn) = crate::codex::make_session_and_context().await;
    let parent_session = Arc::new(parent_session);
    let parent_turn = Arc::new(parent_turn);
    let mut spawn_config = crate::config::test_config();
    spawn_config.model_provider.base_url = Some("https://guardian.example.invalid/v2".into());

    manager
        .maybe_spawn_trunk_eagerly(
            &parent_session,
            &parent_turn,
            spawn_config,
            &manager.eager_init_cancel,
        )
        .await;

    let trunk = manager
        .state
        .lock()
        .await
        .trunk
        .clone()
        .expect("existing trunk should be preserved");
    assert!(Arc::ptr_eq(&trunk, &trunk_session));
}

#[tokio::test]
async fn canceled_eager_trunk_init_does_not_cache_trunk() {
    let manager = GuardianReviewSessionManager::default();
    manager.eager_init_cancel.cancel();

    let (parent_session, parent_turn) = crate::codex::make_session_and_context().await;
    manager
        .run_eager_trunk_init_if_needed(
            Arc::new(parent_session),
            Arc::new(parent_turn),
            manager.eager_init_cancel.clone(),
        )
        .await;

    assert!(manager.state.lock().await.trunk.is_none());
}

#[tokio::test]
async fn fork_review_cleanup_shuts_down_guardian_session_on_drop() {
    let manager = GuardianReviewSessionManager::default();
    let (review_session, child_shutdown_rx) = guardian_review_session_with_shutdown_signal().await;
    manager
        .state
        .lock()
        .await
        .active_forks
        .push(Arc::clone(&review_session));

    drop(ForkReviewCleanup::new(
        Arc::clone(&manager.state),
        Arc::clone(&review_session),
    ));

    tokio::time::timeout(Duration::from_secs(1), child_shutdown_rx)
        .await
        .expect("dropped fork cleanup should shut down guardian session")
        .expect("guardian shutdown signal");
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_times_out_before_future_completes() {
    let outcome = run_before_review_deadline(
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_aborts_when_cancelled() {
    let cancel_token = CancellationToken::new();
    let canceller = cancel_token.clone();
    drop(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        canceller.cancel();
    }));

    let outcome = run_before_review_deadline(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&cancel_token),
        std::future::pending::<()>(),
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::Aborted)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_timeout() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        &cancel_token,
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_abort() {
    let external_cancel = CancellationToken::new();
    let external_canceller = external_cancel.clone();
    let cancel_token = CancellationToken::new();
    drop(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        external_canceller.cancel();
    }));

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&external_cancel),
        &cancel_token,
        std::future::pending::<()>(),
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::Aborted)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &cancel_token,
        async { 42usize },
    )
    .await;

    assert_eq!(outcome.unwrap(), 42);
    assert!(!cancel_token.is_cancelled());
}
