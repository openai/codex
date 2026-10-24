use super::ToolBatchTiming;
use codex_analytics::TurnProfile;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_critical_path::CriticalPath;
use codex_utils_critical_path::PathDurations;
use codex_utils_critical_path::WorkKind;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::TurnProfilePhase;
use super::TurnProfileState;
use super::TurnTimingState;
use super::response_item_records_turn_ttft;
use crate::ResponseEvent;

#[tokio::test]
async fn turn_timing_state_records_ttft_only_once_per_turn() {
    let state = TurnTimingState::default();
    assert_eq!(
        state
            .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
            .await,
        None
    );

    state.mark_turn_started(Instant::now()).await;
    assert_eq!(
        state
            .record_ttft_for_response_event(&ResponseEvent::Created { response_id: None })
            .await,
        None
    );
    assert!(
        state
            .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
            .await
            .is_some()
    );
    assert_eq!(
        state
            .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("again".to_string()))
            .await,
        None
    );
}

#[tokio::test]
async fn turn_timing_state_records_ttfm_independently_of_ttft() {
    let state = TurnTimingState::default();
    state.mark_turn_started(Instant::now()).await;

    assert!(
        state
            .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
            .await
            .is_some()
    );
    assert!(
        state
            .record_ttfm_for_turn_item(&TurnItem::AgentMessage(AgentMessageItem {
                id: "msg-1".to_string(),
                content: Vec::new(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            }))
            .await
            .is_some()
    );
    assert_eq!(
        state
            .record_ttfm_for_turn_item(&TurnItem::AgentMessage(AgentMessageItem {
                id: "msg-2".to_string(),
                content: Vec::new(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            }))
            .await,
        None
    );
}

#[tokio::test]
async fn turn_timing_state_records_turn_started_epoch_millis() {
    let state = TurnTimingState::default();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis();

    let started_at_unix_ms = state.mark_turn_started(Instant::now()).await;

    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis();
    assert!(u128::try_from(started_at_unix_ms).is_ok_and(|ms| before <= ms && ms <= after));
    assert_eq!(
        state.started_at_unix_secs().await,
        Some(started_at_unix_ms / 1000)
    );
}

#[tokio::test]
async fn turn_timing_state_tracks_concurrent_items_and_preserves_first_start() {
    let state = TurnTimingState::default();

    state
        .record_item_started("first".to_string(), /*started_at_ms*/ 100)
        .await;
    state
        .record_item_started("second".to_string(), /*started_at_ms*/ 200)
        .await;
    assert_eq!(
        state
            .record_item_started("first".to_string(), /*started_at_ms*/ 300)
            .await,
        100
    );

    assert_eq!(state.take_item_started("second").await, Some(200));
    assert_eq!(state.take_item_started("first").await, Some(100));
}

#[tokio::test]
async fn turn_timing_state_preserves_in_flight_items_after_turn_completion() {
    let state = TurnTimingState::default();
    state
        .record_item_started("first".to_string(), /*started_at_ms*/ 100)
        .await;

    state.mark_turn_started(Instant::now()).await;
    assert_eq!(state.take_item_started("first").await, None);

    state
        .record_item_started("second".to_string(), /*started_at_ms*/ 200)
        .await;
    let _ = state.complete_profile_and_duration_ms().await;
    assert_eq!(state.take_item_started("second").await, Some(200));
}

#[test]
fn response_item_records_turn_ttft_for_first_output_signals() {
    assert!(response_item_records_turn_ttft(
        &ResponseItem::FunctionCall {
            status: None,
            encrypted_content: None,
            id: None,
            name: "shell".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        }
    ));
    assert!(response_item_records_turn_ttft(
        &ResponseItem::CustomToolCall {
            encrypted_content: None,
            id: None,
            status: None,
            call_id: "call-2".to_string(),
            name: "custom".to_string(),
            namespace: None,
            input: "echo hi".to_string(),
            internal_chat_message_metadata_passthrough: None,
        }
    ));
    assert!(response_item_records_turn_ttft(&ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            annotations: None,
            logprobs: None,
            text: "hello".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }));
}

#[test]
fn response_item_records_turn_ttft_ignores_empty_non_output_items() {
    assert!(!response_item_records_turn_ttft(&ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            annotations: None,
            logprobs: None,
            text: String::new(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }));
    assert!(!response_item_records_turn_ttft(
        &ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
            internal_chat_message_metadata_passthrough: None,
        }
    ));
}

#[test]
fn turn_profile_breaks_down_sampling_blocking_and_retry_overhead() {
    let started_at = Instant::now();
    let mut state = TurnProfileState::default();
    state.start(started_at);

    let _ = state.begin_sampling(started_at + Duration::from_millis(100));
    state.end_phase(
        started_at + Duration::from_millis(600),
        TurnProfilePhase::Sampling,
    );
    let _ = state.begin_tool_blocking(started_at + Duration::from_millis(600));
    state.end_phase(
        started_at + Duration::from_millis(900),
        TurnProfilePhase::ToolBlocking,
    );
    state.record_sampling_retry();
    let _ = state.begin_sampling(started_at + Duration::from_millis(1_000));
    state.end_phase(
        started_at + Duration::from_millis(1_200),
        TurnProfilePhase::Sampling,
    );

    assert_eq!(
        state.complete(started_at + Duration::from_millis(1_300)),
        TurnProfile {
            critical_path: codex_analytics::TurnCriticalPath::Disabled,
            before_first_sampling_ms: 100,
            sampling_ms: 700,
            compaction_ms: 0,
            between_sampling_overhead_ms: 100,
            tool_blocking_ms: 300,
            after_last_sampling_ms: 100,
            sampling_request_count: 2,
            sampling_retry_count: 1,
            tools_change_count: 0,
        }
    );
}

#[test]
fn turn_profile_counts_compaction_as_an_exclusive_phase() {
    let started_at = Instant::now();
    let mut state = TurnProfileState {
        path: Some(CriticalPath::default()),
        capture_enabled: true,
        ..Default::default()
    };
    state.start(started_at);

    let _ = state.begin_compaction(started_at + Duration::from_millis(100));
    state.end_phase(
        started_at + Duration::from_millis(300),
        TurnProfilePhase::Compaction,
    );
    let _ = state.begin_sampling(started_at + Duration::from_millis(400));
    state.end_phase(
        started_at + Duration::from_millis(700),
        TurnProfilePhase::Sampling,
    );
    let _ = state.begin_compaction(started_at + Duration::from_millis(800));

    assert_eq!(
        state.complete(started_at + Duration::from_millis(1_100)),
        TurnProfile {
            critical_path: codex_analytics::TurnCriticalPath::Complete(
                codex_analytics::TurnCriticalPathDurations {
                    duration_ms: 1100,
                    inference_ms: 800,
                    auto_review_ms: 0,
                    user_wait_ms: 0,
                    tools_ms: 0,
                    overhead_ms: 300,
                    unknown_ms: 0,
                }
            ),
            before_first_sampling_ms: 200,
            sampling_ms: 300,
            compaction_ms: 500,
            between_sampling_overhead_ms: 0,
            tool_blocking_ms: 0,
            after_last_sampling_ms: 100,
            sampling_request_count: 1,
            sampling_retry_count: 0,
            tools_change_count: 0,
        }
    );
    let totals = state
        .path
        .as_ref()
        .unwrap()
        .durations(started_at, started_at + Duration::from_millis(1_100));
    assert_eq!(totals.inference, Duration::from_millis(800));
    assert_eq!(totals.overhead, Duration::from_millis(300));
    assert_eq!(totals.user_wait, Duration::ZERO);
}

#[tokio::test]
async fn turn_profile_and_duration_share_a_completion_instant() {
    let state = TurnTimingState::default();
    state
        .mark_turn_started(Instant::now() - Duration::from_millis(100))
        .await;

    state.enable_critical_path();
    let result = state.complete_profile_and_duration_ms().await;
    assert_eq!(state.complete_profile_and_duration_ms().await, result);
    let (_, duration_ms, profile) = result;
    assert_eq!(
        profile.critical_path,
        codex_analytics::TurnCriticalPath::Complete(codex_analytics::TurnCriticalPathDurations {
            duration_ms: duration_ms.unwrap() as u64,
            inference_ms: 0,
            auto_review_ms: 0,
            user_wait_ms: 0,
            tools_ms: 0,
            overhead_ms: duration_ms.unwrap() as u64,
            unknown_ms: 0,
        })
    );
    let classified_ms = profile.before_first_sampling_ms
        + profile.sampling_ms
        + profile.compaction_ms
        + profile.between_sampling_overhead_ms
        + profile.tool_blocking_ms
        + profile.after_last_sampling_ms;

    assert_eq!(
        duration_ms,
        Some(i64::try_from(classified_ms).expect("profile duration should fit i64"))
    );
}

#[test]
fn capture_preserves_phases_and_freezes_missing_dependencies() {
    let started_at = Instant::now();
    let time_after_turn_start = |millis| started_at + Duration::from_millis(millis);
    let mut state = TurnProfileState::default();
    state.start(started_at);
    assert!(state.path_at(time_after_turn_start(10)).is_none());
    state.capture_enabled = true;
    state.path = Some(CriticalPath::default());
    assert!(state.begin_sampling(time_after_turn_start(10)));
    state.end_phase(time_after_turn_start(20), TurnProfilePhase::Sampling);
    assert!(state.begin_tool_blocking(time_after_turn_start(20)));
    state.end_phase(time_after_turn_start(50), TurnProfilePhase::ToolBlocking);
    let profile = state.complete(time_after_turn_start(60));
    assert_eq!(
        state
            .path
            .as_ref()
            .unwrap()
            .durations(started_at, time_after_turn_start(60)),
        PathDurations {
            inference: Duration::from_millis(10),
            unknown: Duration::from_millis(30),
            overhead: Duration::from_millis(20),
            ..PathDurations::default()
        }
    );
    assert_eq!(
        profile.critical_path,
        codex_analytics::TurnCriticalPath::Complete(codex_analytics::TurnCriticalPathDurations {
            duration_ms: 60,
            inference_ms: 10,
            auto_review_ms: 0,
            user_wait_ms: 0,
            tools_ms: 0,
            overhead_ms: 20,
            unknown_ms: 30,
        })
    );
    let frozen_path = state.path.clone();
    state.end_phase(time_after_turn_start(70), TurnProfilePhase::ToolBlocking);
    assert_eq!(state.complete(time_after_turn_start(80)), profile);
    assert_eq!(state.path, frozen_path);
}

#[tokio::test]
async fn invalidation_and_late_guard_drop_cannot_restore_a_frozen_path() {
    let timing = Arc::new(TurnTimingState::default());
    timing.enable_critical_path();
    timing.mark_turn_started(Instant::now()).await;
    let guard = timing.begin_tool_blocking(Some(ToolBatchTiming::default()));
    timing.invalidate_critical_path();
    let (_, _, profile) = timing.complete_profile_and_duration_ms().await;
    assert_eq!(
        profile.critical_path,
        codex_analytics::TurnCriticalPath::Incomplete
    );
    timing.enable_critical_path();
    drop(guard);
    assert!(timing.snapshot_path(Instant::now()).is_none());
}

#[tokio::test]
async fn completed_batch_guard_retains_its_path_across_turns() {
    let started_at = Instant::now() - Duration::from_secs(2);
    let previous_turn = TurnTimingState::default();
    let previous_batch = ToolBatchTiming::default();
    let previous_call = previous_turn.direct_tool_timings.start_turn_direct_call(
        "reviewed",
        "turn",
        previous_batch.clone(),
        started_at,
        Some(CriticalPath::default()),
    );
    drop(previous_turn);
    let timing = Arc::new(TurnTimingState::default());
    timing.enable_critical_path();
    timing.mark_turn_started(started_at).await;
    let batch = ToolBatchTiming::default();
    let review_path = CriticalPath::default().work(
        started_at,
        started_at + Duration::from_secs(1),
        WorkKind::AutoReview,
    );
    let call = timing.direct_tool_timings.start_turn_direct_call(
        "reviewed",
        "turn",
        batch.clone(),
        started_at + Duration::from_secs(1),
        Some(review_path),
    );
    // A late guard from an earlier turn cannot finish or invalidate the reused ID.
    drop(previous_call);
    assert!(previous_batch.completed().unwrap().path.is_some());
    assert!(batch.completed().is_none());
    let guard = timing.begin_tool_blocking(Some(batch));
    drop(call);
    drop(guard);
    timing.complete_profile_and_duration_ms().await;
    let path = timing.snapshot_path(Instant::now()).unwrap();
    let totals = path.durations(started_at, path.end().unwrap());
    assert_eq!(totals.auto_review, Duration::from_secs(1));
    assert_eq!(totals.unknown, Duration::ZERO);
    assert!(totals.tools >= Duration::from_secs(1));
}

#[test]
fn critical_path_report_retains_fractional_millisecond_rounding_and_compaction() {
    let start = Instant::now();
    let mut state = TurnProfileState {
        capture_enabled: true,
        path: Some(CriticalPath::default()),
        ..Default::default()
    };
    state.start(start);
    state.begin_compaction(start);
    let end = start + Duration::from_nanos(1_999_999);
    state.end_phase(end, TurnProfilePhase::Compaction);
    let profile = state.complete(end);
    assert_eq!(
        profile.critical_path,
        codex_analytics::TurnCriticalPath::Complete(codex_analytics::TurnCriticalPathDurations {
            duration_ms: 1,
            inference_ms: 1,
            auto_review_ms: 0,
            user_wait_ms: 0,
            tools_ms: 0,
            overhead_ms: 0,
            unknown_ms: 0,
        })
    );
}
