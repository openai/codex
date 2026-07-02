use super::*;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;

fn token_budget_reminder(turn_id: &str) -> ResponseItem {
    let mut reminder = ContextualUserFragment::into(TokenBudgetReminder::new(
        "reconciled reminder with {n_remaining} tokens",
        /*n_remaining*/ 25,
    ));
    reminder.set_turn_id_if_missing(turn_id);
    reminder
}

#[tokio::test]
async fn reconcile_imported_same_window_reminder_restores_one_shot_latch() {
    let (session, _turn_context) = make_session_and_context().await;
    let first_history = model_history_for_turn("first user", "first assistant");
    session.replace_history(first_history.clone(), None).await;
    let mut rollout = completed_turn("turn-1", "first user", "first assistant");
    set_known_persisted_history(&session, &rollout).await;
    let reminder = token_budget_reminder("turn-2");
    let mut imported_turn = completed_turn("turn-2", "external user", "external assistant");
    imported_turn.insert(
        imported_turn.len() - 1,
        RolloutItem::ResponseItem(reminder.clone()),
    );
    rollout.extend(imported_turn);

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    let mut expected_history = first_history;
    expected_history.extend(model_history_for_turn(
        "external user",
        "external assistant",
    ));
    expected_history.push(reminder);
    let mut state = session.state.lock().await;
    assert_eq!(state.history.raw_items(), expected_history);
    assert!(
        !state.claim_token_budget_reminder(),
        "an imported current-window reminder must remain one-shot after reconciliation"
    );
}

#[tokio::test]
async fn cold_restore_from_compacted_history_restores_reminder_latch() {
    let (session, _turn_context) = make_session_and_context().await;
    let reminder = token_budget_reminder("compact-turn");
    let window_ids = new_compaction_window_ids();
    let rollout = vec![compacted_item(
        vec![reminder.clone()],
        /*window_number*/ 7,
        window_ids,
    )];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: session.thread_id(),
            history: Arc::new(rollout),
            rollout_path: None,
        }))
        .await;

    let mut state = session.state.lock().await;
    assert_eq!(state.history.raw_items(), vec![reminder]);
    assert_eq!(state.auto_compact_window_number(), 7);
    assert_eq!(state.auto_compact_window_ids(), window_ids);
    assert!(
        !state.claim_token_budget_reminder(),
        "a reminder surviving the compacted current window must remain one-shot after cold restore"
    );
}
