use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::items::HookPromptFragment;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;

use super::ExplicitTurnState;
use super::RolloutTurnLifecycleTracker;

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

fn turn_aborted(turn_id: Option<&str>) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
        turn_id: turn_id.map(str::to_string),
        reason: TurnAbortReason::Interrupted,
        completed_at: None,
        duration_ms: None,
    }))
}

fn user_message(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: message.to_string(),
        ..Default::default()
    }))
}

fn agent_message(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
        message: message.to_string(),
        phase: None,
        memory_citation: None,
    }))
}

fn compacted() -> RolloutItem {
    RolloutItem::Compacted(CompactedItem {
        message: String::new(),
        replacement_history: None,
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    })
}

fn hook_prompt() -> RolloutItem {
    let fragments = [HookPromptFragment::from_single_hook(
        "hook guidance",
        "hook-run-1",
    )];
    RolloutItem::ResponseItem(build_hook_prompt_message(&fragments).expect("hook prompt message"))
}

fn rollback(num_turns: u32) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
        num_turns,
    }))
}

fn observe(tracker: &mut RolloutTurnLifecycleTracker, items: &[RolloutItem]) {
    for item in items {
        tracker.handle_rollout_item(item);
    }
}

fn current_turn(tracker: &RolloutTurnLifecycleTracker) -> Option<(&str, usize, ExplicitTurnState)> {
    tracker
        .current_explicit_turn()
        .map(|turn| (turn.turn_id.as_str(), turn.rollout_start_index, turn.state))
}
#[test]
fn records_raw_rollout_index_for_explicit_turn_start() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            RolloutItem::ResponseItem(ResponseItem::Other),
            RolloutItem::ResponseItem(ResponseItem::Other),
            turn_started("turn-a"),
        ],
    );

    assert_eq!(
        current_turn(&tracker),
        Some(("turn-a", 2, ExplicitTurnState::InProgress))
    );
}

#[test]
fn complete_closes_while_abort_and_error_retain_terminal_current_turn() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[turn_started("turn-a"), turn_aborted(Some("turn-a"))],
    );
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-a", 0, ExplicitTurnState::Terminal))
    );

    tracker.handle_rollout_item(&turn_complete("turn-a"));
    assert_eq!(tracker.current_explicit_turn(), None);

    tracker.handle_rollout_item(&turn_started("turn-b"));
    tracker.handle_rollout_item(&RolloutItem::EventMsg(EventMsg::Error(ErrorEvent {
        message: "failed".to_string(),
        codex_error_info: None,
    })));
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 3, ExplicitTurnState::Terminal))
    );
}

#[test]
fn second_start_finishes_and_replaces_current_turn() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[turn_started("turn-a"), turn_started("turn-b")],
    );

    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 1, ExplicitTurnState::InProgress))
    );
}

#[test]
fn late_historical_ids_do_not_affect_current_but_unknown_ids_do() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            turn_complete("turn-a"),
            turn_started("turn-b"),
            turn_complete("turn-a"),
            turn_aborted(Some("turn-a")),
        ],
    );
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 2, ExplicitTurnState::InProgress))
    );

    tracker.handle_rollout_item(&turn_aborted(Some("unknown")));
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 2, ExplicitTurnState::Terminal))
    );

    tracker.handle_rollout_item(&turn_complete("unknown"));
    assert_eq!(tracker.current_explicit_turn(), None);
}

#[test]
fn rollback_zero_finishes_current_and_rolled_back_ids_become_unknown() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            rollback(/*num_turns*/ 0),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 2, ExplicitTurnState::InProgress))
    );

    observe(
        &mut tracker,
        &[
            rollback(/*num_turns*/ 1),
            turn_started("turn-c"),
            turn_complete("turn-b"),
        ],
    );
    assert_eq!(tracker.current_explicit_turn(), None);
}

#[test]
fn implicit_turn_placeholders_keep_rollback_late_id_matching_aligned() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            turn_complete("turn-a"),
            user_message("legacy turn"),
            rollback(/*num_turns*/ 1),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 4, ExplicitTurnState::InProgress))
    );

    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            user_message("legacy turn"),
            turn_started("turn-a"),
            turn_complete("turn-a"),
            rollback(/*num_turns*/ 1),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );
    assert_eq!(tracker.current_explicit_turn(), None);
}

#[test]
fn non_user_materialized_turn_adds_an_implicit_rollback_placeholder() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            turn_complete("turn-a"),
            agent_message("legacy response"),
            rollback(/*num_turns*/ 1),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );

    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 4, ExplicitTurnState::InProgress))
    );
}

#[test]
fn legacy_current_turn_events_add_implicit_rollback_placeholders() {
    let events = [
        EventMsg::ViewImageToolCall(
            serde_json::from_value(serde_json::json!({
                "call_id": "image-1",
                "path": "file:///tmp/image.png",
            }))
            .expect("valid view image event"),
        ),
        EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
            call_id: "dynamic-1".to_string(),
            turn_id: String::new(),
            started_at_ms: 0,
            namespace: None,
            tool: "lookup".to_string(),
            arguments: serde_json::json!({}),
        }),
    ];

    for event in events {
        let mut tracker = RolloutTurnLifecycleTracker::new();
        observe(
            &mut tracker,
            &[
                turn_started("turn-a"),
                turn_complete("turn-a"),
                RolloutItem::EventMsg(event),
                rollback(/*num_turns*/ 1),
                turn_started("turn-b"),
                turn_complete("turn-a"),
            ],
        );
        assert_eq!(
            current_turn(&tracker),
            Some(("turn-b", 4, ExplicitTurnState::InProgress))
        );
    }
}

#[test]
fn first_user_message_reuses_compaction_only_turn_and_second_starts_another() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            turn_complete("turn-a"),
            compacted(),
            user_message("first legacy turn"),
            user_message("second legacy turn"),
            rollback(/*num_turns*/ 3),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );

    assert_eq!(tracker.current_explicit_turn(), None);
}

#[test]
fn hook_prompt_adds_an_implicit_rollback_placeholder() {
    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(
        &mut tracker,
        &[
            turn_started("turn-a"),
            turn_complete("turn-a"),
            hook_prompt(),
            rollback(/*num_turns*/ 1),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );

    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 4, ExplicitTurnState::InProgress))
    );
}

#[test]
fn hook_prompt_materializes_compaction_slot_before_user_starts_another() {
    let items_before_rollback = [
        turn_started("turn-a"),
        turn_complete("turn-a"),
        compacted(),
        hook_prompt(),
        user_message("legacy turn"),
    ];

    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(&mut tracker, &items_before_rollback);
    observe(
        &mut tracker,
        &[
            rollback(/*num_turns*/ 2),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );
    assert_eq!(
        current_turn(&tracker),
        Some(("turn-b", 6, ExplicitTurnState::InProgress))
    );

    let mut tracker = RolloutTurnLifecycleTracker::new();
    observe(&mut tracker, &items_before_rollback);
    observe(
        &mut tracker,
        &[
            rollback(/*num_turns*/ 3),
            turn_started("turn-b"),
            turn_complete("turn-a"),
        ],
    );
    assert_eq!(tracker.current_explicit_turn(), None);
}
