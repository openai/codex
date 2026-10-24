#![cfg(test)]
//! Keeps uninstrumented review work out of complete critical-path captures.

use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Instant;

#[test_case::test_case(true; "capture_enabled")]
#[test_case::test_case(false; "capture_disabled")]
#[tokio::test]
async fn uncaptured_review_invalidates_before_polling(critical_path_timing: bool) {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    Arc::make_mut(&mut turn.config)
        .features
        .set_enabled(
            codex_features::Feature::CriticalPathTiming,
            critical_path_timing,
        )
        .unwrap();
    let turn = Arc::new(turn);
    turn.turn_timing_state.enable_critical_path();
    turn.turn_timing_state
        .mark_turn_started(Instant::now())
        .await;
    assert!(
        turn.turn_timing_state
            .snapshot_path(Instant::now())
            .is_some()
    );
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();
    let decision = crate::guardian::decide_approval(
        Arc::new(session),
        &turn,
        "review".to_string(),
        crate::guardian::GuardianApprovalRequest::ApplyPatch {
            id: "call".to_string(),
            environment_id: "local".to_string(),
            cwd: codex_utils_path_uri::PathUri::parse("file:///tmp").unwrap(),
            files: Vec::new(),
            patch: String::new(),
        },
        crate::tools::sandboxing::ApprovalRequestReasons::default(),
        crate::guardian::GuardianReviewOptions {
            require_guardian: true,
            plugin_attribution_override: None,
            approval_request_source: codex_analytics::GuardianApprovalRequestSource::MainTurn,
            external_cancel: Some(cancellation),
            require_synchronous_review: false,
        },
    );
    // No polling or worker scheduling is needed to protect a finalizing turn.
    assert_eq!(
        turn.turn_timing_state
            .snapshot_path(Instant::now())
            .is_none(),
        critical_path_timing
    );
    assert_eq!(
        decision.await,
        Some(codex_protocol::protocol::ReviewDecision::Abort)
    );
}
