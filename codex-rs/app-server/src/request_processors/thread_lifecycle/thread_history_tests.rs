use super::*;

#[test]
fn app_history_requires_event_only_rollback_marker_to_remove_turn() {
    let mut rollout = vec![RolloutItem::EventMsg(EventMsg::TurnStarted(
        TurnStartedEvent {
            turn_id: "event-only-turn".to_string(),
            trace_id: None,
            started_at: Some(1),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        },
    ))];
    rollout.push(RolloutItem::EventMsg(EventMsg::TurnComplete(
        TurnCompleteEvent {
            turn_id: "event-only-turn".to_string(),
            last_agent_message: None,
            completed_at: Some(2),
            duration_ms: Some(1_000),
            time_to_first_token_ms: None,
        },
    )));

    let stale_turns = build_api_turns_from_rollout_items(&rollout);
    assert_eq!(stale_turns.len(), 1);
    assert_eq!(stale_turns[0].id, "event-only-turn");

    rollout.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
        ThreadRolledBackEvent { num_turns: 1 },
    )));
    assert!(
        build_api_turns_from_rollout_items(&rollout).is_empty(),
        "a resume may expose this history only after the proven rollback marker removes the event-only turn"
    );
}
