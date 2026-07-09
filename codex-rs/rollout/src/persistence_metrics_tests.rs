use codex_protocol::ThreadId;
use codex_protocol::items::EnteredReviewModeItem;
use codex_protocol::items::ExitedReviewModeItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EnteredReviewModeEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use pretty_assertions::assert_eq;

use super::CompletedTurnMeasurement;
use super::TurnMeasurementState;
use super::TurnOutcome;
use super::TurnSizeTotals;
use super::is_thread_sampled;
use super::measure_and_filter_rollout_items;
use super::update_turn_measurements;

fn retained_message(text: &str) -> RolloutItem {
    RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

fn turn_started(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
}

fn turn_complete(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        completed_at: None,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

fn turn_aborted(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
        turn_id: Some(turn_id.to_string()),
        reason: TurnAbortReason::Interrupted,
        completed_at: None,
        duration_ms: None,
    }))
}

fn update_for_batch(
    state: &mut TurnMeasurementState,
    items: &[RolloutItem],
) -> super::TurnMeasurementUpdate {
    let (_, measurement) = measure_and_filter_rollout_items(items, ThreadHistoryMode::Legacy);
    update_turn_measurements(state, items, &measurement)
}

#[test]
fn thread_sampling_is_stable_and_selects_whole_threads() {
    let mut sampled = None;
    let mut unsampled = None;
    for value in 0..10_000_u128 {
        let thread_id = ThreadId::from_string(&format!("00000000-0000-0000-0000-{value:012x}"))
            .expect("valid thread id");
        if is_thread_sampled(thread_id) {
            sampled.get_or_insert(thread_id);
        } else {
            unsampled.get_or_insert(thread_id);
        }
        if sampled.is_some() && unsampled.is_some() {
            break;
        }
    }

    let sampled = sampled.expect("at least one sampled thread");
    let unsampled = unsampled.expect("at least one unsampled thread");
    assert!(is_thread_sampled(sampled));
    assert!(is_thread_sampled(sampled));
    assert!(!is_thread_sampled(unsampled));
    assert!(!is_thread_sampled(unsampled));
}

#[test]
fn mixed_batch_reports_exact_policy_counts_and_bytes() {
    let kept = retained_message("hello");
    let dropped = RolloutItem::ResponseItem(ResponseItem::Other);
    let items = vec![kept.clone(), dropped.clone()];

    let (persisted, measurement) =
        measure_and_filter_rollout_items(&items, ThreadHistoryMode::Legacy);
    let kept_bytes = serde_json::to_vec(&kept)
        .expect("serialize kept item")
        .len() as u64;
    let dropped_bytes = serde_json::to_vec(&dropped)
        .expect("serialize dropped item")
        .len() as u64;

    assert_eq!(
        serde_json::to_value(persisted).expect("serialize persisted items"),
        serde_json::to_value([kept]).expect("serialize expected items")
    );
    assert_eq!(measurement.pre_filter.items, 2);
    assert_eq!(
        measurement.pre_filter.payload_bytes,
        kept_bytes + dropped_bytes
    );
    assert_eq!(measurement.post_filter.items, 1);
    assert_eq!(measurement.post_filter.payload_bytes, kept_bytes);
    assert_eq!(measurement.items[0].payload_bytes, Some(kept_bytes));
    assert_eq!(measurement.items[1].payload_bytes, Some(dropped_bytes));
    assert_eq!(measurement.items[0].rollout_item_type, "response.message");
    assert_eq!(measurement.items[1].rollout_item_type, "response.other");
}

#[test]
fn retained_items_are_byte_identical() {
    let item = retained_message("a moderately sized payload");
    let (persisted, measurement) =
        measure_and_filter_rollout_items(std::slice::from_ref(&item), ThreadHistoryMode::Legacy);

    assert_eq!(
        serde_json::to_vec(&persisted[0]).expect("serialize persisted item"),
        serde_json::to_vec(&item).expect("serialize candidate item")
    );
    assert_eq!(
        measurement.post_filter.payload_bytes,
        measurement.items[0].payload_bytes.expect("payload bytes")
    );
}

#[test]
fn turn_measurements_span_batches_and_include_items_before_start() {
    let first_turn = vec![
        retained_message("first prompt"),
        turn_started("turn-1"),
        retained_message("first response"),
        RolloutItem::ResponseItem(ResponseItem::Other),
        turn_complete("turn-1"),
    ];
    let second_turn = vec![
        retained_message("second prompt"),
        turn_started("turn-2"),
        retained_message("second response"),
        turn_aborted("turn-2"),
    ];
    let (_, first_expected) =
        measure_and_filter_rollout_items(&first_turn, ThreadHistoryMode::Legacy);
    let (_, second_expected) =
        measure_and_filter_rollout_items(&second_turn, ThreadHistoryMode::Legacy);
    let batches = [
        first_turn[..1].to_vec(),
        first_turn[1..3].to_vec(),
        vec![
            first_turn[3].clone(),
            first_turn[4].clone(),
            second_turn[0].clone(),
        ],
        second_turn[1..].to_vec(),
    ];

    let mut state = TurnMeasurementState::default();
    let mut completed = Vec::new();
    let mut boundary_errors = Vec::new();
    for batch in batches {
        let update = update_for_batch(&mut state, &batch);
        completed.extend(update.completed);
        boundary_errors.extend(update.boundary_errors);
    }

    assert_eq!(
        completed,
        vec![
            CompletedTurnMeasurement {
                totals: TurnSizeTotals {
                    pre_filter: first_expected.pre_filter,
                    post_filter: first_expected.post_filter,
                },
                outcome: TurnOutcome::Completed,
            },
            CompletedTurnMeasurement {
                totals: TurnSizeTotals {
                    pre_filter: second_expected.pre_filter,
                    post_filter: second_expected.post_filter,
                },
                outcome: TurnOutcome::Aborted,
            },
        ]
    );
    assert_eq!(boundary_errors, Vec::<&str>::new());
    assert_eq!(state, TurnMeasurementState::default());
}

#[test]
fn invalid_turn_boundaries_reset_partial_measurements() {
    let mut state = TurnMeasurementState::default();
    let unmatched_completion = vec![retained_message("orphan"), turn_complete("turn-1")];
    let update = update_for_batch(&mut state, &unmatched_completion);

    assert_eq!(update.completed, Vec::new());
    assert_eq!(update.boundary_errors, vec!["event.turn_complete"]);
    assert_eq!(state, TurnMeasurementState::default());

    let replacement = vec![
        turn_started("turn-1"),
        retained_message("discarded partial turn"),
        turn_started("turn-2"),
        retained_message("retained turn"),
        turn_complete("turn-2"),
    ];
    let (_, expected) =
        measure_and_filter_rollout_items(&replacement[2..], ThreadHistoryMode::Legacy);
    let update = update_for_batch(&mut state, &replacement);

    assert_eq!(
        update.completed,
        vec![CompletedTurnMeasurement {
            totals: TurnSizeTotals {
                pre_filter: expected.pre_filter,
                post_filter: expected.post_filter,
            },
            outcome: TurnOutcome::Completed,
        }]
    );
    assert_eq!(update.boundary_errors, vec!["event.turn_started"]);
    assert_eq!(state, TurnMeasurementState::default());
}

#[test]
fn item_completion_persistence_depends_on_history_mode() {
    let item = RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::default(),
        turn_id: "turn".to_string(),
        item: TurnItem::UserMessage(UserMessageItem {
            id: "item".to_string(),
            client_id: None,
            content: Vec::new(),
        }),
        completed_at_ms: 0,
    }));

    let (_, legacy_measurement) =
        measure_and_filter_rollout_items(std::slice::from_ref(&item), ThreadHistoryMode::Legacy);

    assert_eq!(
        legacy_measurement.items[0].rollout_item_type,
        "event.item_completed.user_message"
    );
    assert_eq!(
        legacy_measurement.items[0].decision,
        super::PersistenceDecision::Dropped
    );

    let (persisted, paginated_measurement) =
        measure_and_filter_rollout_items(std::slice::from_ref(&item), ThreadHistoryMode::Paginated);

    assert_eq!(
        serde_json::to_value(persisted).expect("serialize persisted items"),
        serde_json::to_value([item]).expect("serialize expected items")
    );
    assert_eq!(
        paginated_measurement.items[0].decision,
        super::PersistenceDecision::Kept
    );
}

#[test]
fn review_mode_persistence_depends_on_history_mode() {
    let completed_items = vec![
        RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::default(),
            turn_id: "turn".to_string(),
            item: TurnItem::EnteredReviewMode(EnteredReviewModeItem {
                id: "entered-review".to_string(),
                target: ReviewTarget::Custom {
                    instructions: "review this".to_string(),
                },
                user_facing_hint: "Review requested.".to_string(),
            }),
            completed_at_ms: 0,
        })),
        RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::default(),
            turn_id: "turn".to_string(),
            item: TurnItem::ExitedReviewMode(ExitedReviewModeItem {
                id: "exited-review".to_string(),
                review_output: None,
            }),
            completed_at_ms: 0,
        })),
    ];
    let legacy_events = vec![
        RolloutItem::EventMsg(EventMsg::EnteredReviewMode(EnteredReviewModeEvent {
            target: ReviewTarget::Custom {
                instructions: "review this".to_string(),
            },
            user_facing_hint: Some("Review requested.".to_string()),
            turn_id: Some("turn".to_string()),
            item_id: Some("entered-review".to_string()),
        })),
        RolloutItem::EventMsg(EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            turn_id: Some("turn".to_string()),
            item_id: Some("exited-review".to_string()),
            review_output: None,
        })),
    ];
    let items = completed_items
        .iter()
        .chain(&legacy_events)
        .cloned()
        .collect::<Vec<_>>();

    let (persisted_legacy, _) = measure_and_filter_rollout_items(&items, ThreadHistoryMode::Legacy);
    assert_eq!(
        serde_json::to_value(persisted_legacy).expect("serialize persisted items"),
        serde_json::to_value(legacy_events).expect("serialize expected items")
    );

    let (persisted_paginated, _) =
        measure_and_filter_rollout_items(&items, ThreadHistoryMode::Paginated);
    assert_eq!(
        serde_json::to_value(persisted_paginated).expect("serialize persisted items"),
        serde_json::to_value(completed_items).expect("serialize expected items")
    );
}
