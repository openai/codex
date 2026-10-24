#![cfg(test)]
//! Verifies review timing binds to its invocation before scheduling and respects the capture gate.

use codex_direct_tool_timing::ToolBatchTiming;
use codex_utils_critical_path::CriticalPath;
use codex_utils_critical_path::PathDurations;
use core_test_support::PathBufExt;
use core_test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[test_case::test_case(false, false, true; "drop_before_poll")]
#[test_case::test_case(false, true, true; "cancelled_decision_returns")]
#[test_case::test_case(true, false, true; "uncorrelated_network")]
#[test_case::test_case(false, false, false; "runtime_metrics_does_not_start_review_timing")]
#[test_case::test_case(true, false, false; "runtime_metrics_does_not_invalidate_review_timing")]
#[tokio::test]
async fn decision_binds_before_scheduling_and_does_not_follow_reused_ids(
    network: bool,
    await_decision: bool,
    critical_path_timing: bool,
) {
    let (session, mut turn) = crate::session::tests::make_session_and_context().await;
    Arc::make_mut(&mut turn.config)
        .features
        .enable(codex_features::Feature::RuntimeMetrics)
        .unwrap();
    Arc::make_mut(&mut turn.config)
        .features
        .set_enabled(
            codex_features::Feature::CriticalPathTiming,
            critical_path_timing,
        )
        .unwrap();
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    turn.turn_timing_state.enable_critical_path();
    turn.turn_timing_state
        .mark_turn_started(Instant::now())
        .await;
    let timings = &turn.turn_timing_state.direct_tool_timings;
    let batch = ToolBatchTiming::default();
    let original_started_at = Instant::now();
    let original = timings.start_turn_direct_call(
        "call",
        &turn.sub_id,
        batch.clone(),
        original_started_at,
        Some(CriticalPath::default()),
    );
    let request = if network {
        crate::guardian::GuardianApprovalRequest::NetworkAccess {
            id: "network-review".to_string(),
            turn_id: turn.sub_id.clone(),
            environment_id: "local".to_string(),
            target: "example.com:443".to_string(),
            host: "example.com".to_string(),
            protocol: codex_protocol::approvals::NetworkApprovalProtocol::Https,
            port: 443,
            trigger: None,
        }
    } else {
        crate::guardian::GuardianApprovalRequest::ApplyPatch {
            id: "call".to_string(),
            environment_id: "local".to_string(),
            cwd: codex_utils_path_uri::PathUri::parse("file:///tmp").unwrap(),
            files: Vec::new(),
            patch: String::new(),
        }
    };
    assert!(
        timings
            .start_review_for_turn("call", "another-turn")
            .is_none()
    );
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();
    let future = crate::guardian::decide_approval(
        Arc::clone(&session),
        &turn,
        "review".to_string(),
        request,
        crate::tools::sandboxing::ApprovalRequestReasons::default(),
        crate::guardian::GuardianReviewOptions {
            require_guardian: true,
            plugin_attribution_override: None,
            approval_request_source: codex_analytics::GuardianApprovalRequestSource::MainTurn,
            external_cancel: Some(cancellation),
            require_synchronous_review: false,
        },
    );
    assert_eq!(
        turn.turn_timing_state
            .snapshot_path(Instant::now())
            .is_none(),
        network && critical_path_timing
    );
    if await_decision {
        assert_eq!(
            future.await,
            Some(codex_protocol::protocol::ReviewDecision::Abort)
        );
        let completed = original.finish().unwrap();
        let durations = completed
            .path
            .unwrap()
            .durations(original_started_at, completed.ended_at);
        assert!(durations.auto_review > Duration::ZERO);
        return;
    }
    // Finishing before polling observes the review started synchronously by decide_approval.
    let completed = original.finish().unwrap();
    let durations = completed
        .path
        .unwrap()
        .durations(original_started_at, completed.ended_at);
    assert_eq!(
        durations.auto_review > Duration::ZERO,
        !network && critical_path_timing
    );
    drop(original);
    let replacement_started_at = Instant::now();
    let replacement = timings.start_turn_direct_call(
        "call",
        &turn.sub_id,
        batch,
        replacement_started_at,
        Some(CriticalPath::default()),
    );
    drop(future);
    let completed = replacement.finish().unwrap();
    assert_eq!(
        completed
            .path
            .unwrap()
            .durations(replacement_started_at, completed.ended_at),
        PathDurations {
            tools: completed.ended_at.duration_since(replacement_started_at),
            ..PathDurations::default()
        }
    );
}

#[test_case::test_case(true; "owned")]
#[test_case::test_case(false; "ownerless")]
#[tokio::test]
async fn network_review_timing_uses_trigger_owner(owned: bool) {
    let (session, mut turn, _rx) = crate::session::tests::make_session_and_context_with_rx().await;
    Arc::make_mut(&mut Arc::get_mut(&mut turn).expect("unique test turn").config)
        .features
        .enable(codex_features::Feature::CriticalPathTiming)
        .expect("critical-path capture is configurable");
    let timing = &turn.turn_timing_state;
    let start = Instant::now();
    timing.mark_turn_started(start).await;
    timing.enable_critical_path();
    let batch = ToolBatchTiming::default();
    let work = turn
        .turn_timing_state
        .direct_tool_timings
        .start_turn_direct_call(
            "network-tool",
            &turn.sub_id,
            batch.clone(),
            start,
            timing.snapshot_path(start),
        );
    let blocking = timing.begin_tool_blocking(Some(batch));
    let request = super::GuardianApprovalRequest::NetworkAccess {
        id: "network-review".to_string(),
        turn_id: turn.sub_id.clone(),
        environment_id: "local".to_string(),
        target: "https://example.com:443".to_string(),
        host: "example.com".to_string(),
        protocol: codex_protocol::approvals::NetworkApprovalProtocol::Https,
        port: 443,
        trigger: owned.then(|| super::GuardianNetworkAccessTrigger {
            call_id: "network-tool".to_string(),
            tool_name: "shell".to_string(),
            command: vec!["curl".to_string()],
            cwd: test_path_buf("/repo").abs().into(),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
            tty: None,
        }),
    };
    // Binding happens before scheduling, so this also covers cancellation before polling.
    let decision = super::decide_approval(
        Arc::clone(&session),
        &turn,
        "network-review".to_string(),
        request,
        crate::tools::sandboxing::ApprovalRequestReasons {
            approval: None,
            retry: None,
        },
        super::GuardianReviewOptions {
            require_guardian: true,
            plugin_attribution_override: None,
            approval_request_source: codex_analytics::GuardianApprovalRequestSource::MainTurn,
            external_cancel: None,
            require_synchronous_review: false,
        },
    );
    drop(decision);
    drop(work);
    drop(blocking);
    let end = Instant::now();
    let path = timing.snapshot_path(end);
    if owned {
        let totals = path.expect("owned network capture").durations(start, end);
        assert!(totals.auto_review > Duration::ZERO);
        assert_eq!(totals.unknown, Duration::ZERO);
    } else {
        assert_eq!(path, None);
    }
}
