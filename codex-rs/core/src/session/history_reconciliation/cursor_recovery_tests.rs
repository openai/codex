use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn reconcile_persisted_history_rejects_loaded_prefix_shorter_than_cursor() {
    let (session, _turn_context) = make_session_and_context().await;
    let mut local_history = model_history_for_turn("first user", "first assistant");
    local_history.extend(model_history_for_turn("second user", "second assistant"));
    session.replace_history(local_history.clone(), None).await;

    let mut full_rollout = completed_turn("turn-1", "first user", "first assistant");
    full_rollout.extend(completed_turn("turn-2", "second user", "second assistant"));
    set_known_persisted_history(&session, &full_rollout).await;
    let shorter_rollout = completed_turn("turn-1", "first user", "first assistant");

    let outcome = reconcile_idle(&session, &shorter_rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Conflict);
    assert_eq!(session.clone_history().await.raw_items(), local_history);
}

#[test]
fn persisted_cursor_canonicalizes_nested_map_order_and_round_trip() {
    let mut left_env = HashMap::new();
    left_env.insert("ALPHA".to_string(), "1".to_string());
    left_env.insert("BETA".to_string(), "2".to_string());
    let mut right_env = HashMap::new();
    right_env.insert("BETA".to_string(), "2".to_string());
    right_env.insert("ALPHA".to_string(), "1".to_string());
    let left = local_shell_call(left_env);
    let right = local_shell_call(right_env);
    let round_tripped: RolloutItem =
        serde_json::from_slice(&serde_json::to_vec(&left).expect("serialize rollout item"))
            .expect("deserialize rollout item");

    assert_eq!(
        persisted_history_cursor(std::slice::from_ref(&left)),
        persisted_history_cursor(std::slice::from_ref(&right))
    );
    assert_eq!(
        persisted_history_cursor(std::slice::from_ref(&left)),
        persisted_history_cursor(std::slice::from_ref(&round_tripped))
    );
}

#[tokio::test]
async fn persisted_cursor_does_not_advance_for_session_metadata_append() {
    let (session, _turn_context) = make_session_and_context().await;
    let prefix = completed_turn("turn-1", "first user", "first assistant");
    let cursor = persisted_history_cursor(&prefix);
    set_known_persisted_history(&session, &prefix).await;
    let rollout_guard = session.acquire_rollout_persistence_lock().await;

    session
        .note_persisted_non_metadata_items(&rollout_guard, &[session_meta(session.thread_id())])
        .await;

    assert_eq!(
        session.state.lock().await.known_persisted_history_cursor(),
        cursor
    );
}

#[tokio::test]
async fn failed_persisted_append_invalidates_cursor() {
    let (session, _turn_context) = make_session_and_context().await;
    let prefix = completed_turn("turn-1", "first user", "first assistant");
    set_known_persisted_history(&session, &prefix).await;
    invalidate_persisted_history_cursor(
        &session,
        &[RolloutItem::ResponseItem(assistant_message(
            "ambiguous append",
        ))],
    )
    .await;

    assert_eq!(
        session.state.lock().await.known_persisted_history_cursor(),
        None
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .persisted_history_cursor_uncertainty(),
        Some(PersistedHistoryCursorUncertainty::AppendOnly)
    );
}

#[tokio::test]
async fn persisted_cursor_uncertainty_only_upgrades_to_history_rewrite() {
    let (session, _turn_context) = make_session_and_context().await;
    let mut expected_rollout = completed_turn("turn-1", "first user", "first assistant");
    set_known_persisted_history(&session, &expected_rollout).await;
    let rollout_guard = session.acquire_rollout_persistence_lock().await;
    let append = RolloutItem::ResponseItem(assistant_message("ambiguous append"));
    let successful_append = RolloutItem::ResponseItem(assistant_message("successful append"));
    let rollback = RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
        num_turns: 1,
    }));

    session
        .invalidate_persisted_item_cursor(&rollout_guard, std::slice::from_ref(&append))
        .await;
    session
        .note_persisted_non_metadata_items(&rollout_guard, std::slice::from_ref(&successful_append))
        .await;
    session
        .invalidate_persisted_item_cursor(&rollout_guard, std::slice::from_ref(&rollback))
        .await;
    session
        .invalidate_persisted_item_cursor(&rollout_guard, std::slice::from_ref(&append))
        .await;

    assert_eq!(
        session
            .state
            .lock()
            .await
            .persisted_history_cursor_uncertainty(),
        Some(PersistedHistoryCursorUncertainty::HistoryRewrite)
    );
    expected_rollout.extend([append.clone(), successful_append, rollback, append]);
    assert_eq!(
        session
            .state
            .lock()
            .await
            .uncertain_expected_persisted_history_cursor(),
        persisted_history_cursor(&expected_rollout)
    );
}

#[tokio::test]
async fn uncertain_persisted_cursor_never_replaces_valid_in_memory_tail() {
    let (session, _turn_context) = make_session_and_context().await;
    let mut persisted_prefix = completed_turn("turn-1", "first user", "first assistant");
    let local_tail = assistant_message("valid local tail after uncertain append");
    let mut local_history = model_history_for_turn("first user", "first assistant");
    local_history.push(local_tail.clone());
    session.replace_history(local_history.clone(), None).await;
    set_known_persisted_history(&session, &persisted_prefix).await;
    invalidate_persisted_history_cursor(&session, &[RolloutItem::ResponseItem(local_tail.clone())])
        .await;

    let outcome = reconcile_idle(&session, &persisted_prefix).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Conflict);
    assert_eq!(session.clone_history().await.raw_items(), local_history);
    assert_eq!(
        session
            .state
            .lock()
            .await
            .persisted_history_cursor_uncertainty(),
        Some(PersistedHistoryCursorUncertainty::AppendOnly)
    );

    // If a later read proves the exact ambiguous append reached storage, reconciliation can
    // re-establish the cursor without rewriting authoritative in-memory history.
    persisted_prefix.push(RolloutItem::ResponseItem(local_tail));
    let retry_outcome = reconcile_idle(&session, &persisted_prefix).await;
    assert_eq!(retry_outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    {
        let state = session.state.lock().await;
        assert_eq!(
            state.known_persisted_history_cursor(),
            persisted_history_cursor(&persisted_prefix)
        );
        assert_eq!(state.persisted_history_cursor_uncertainty(), None);
    }

    let second_ambiguous_append = assistant_message("second ambiguous append");
    let mut local_history_with_second_append = local_history.clone();
    local_history_with_second_append.push(second_ambiguous_append.clone());
    session
        .replace_history(local_history_with_second_append.clone(), None)
        .await;
    invalidate_persisted_history_cursor(
        &session,
        &[RolloutItem::ResponseItem(second_ambiguous_append.clone())],
    )
    .await;
    persisted_prefix.push(RolloutItem::ResponseItem(second_ambiguous_append));
    persisted_prefix.extend(completed_turn(
        "turn-2",
        "external user",
        "external assistant",
    ));
    let extension_outcome = reconcile_idle(&session, &persisted_prefix).await;
    assert_eq!(
        extension_outcome,
        ThreadHistoryReconciliationOutcome::Refreshed
    );
    let mut expected = local_history_with_second_append;
    expected.extend(model_history_for_turn(
        "external user",
        "external assistant",
    ));
    assert_eq!(session.clone_history().await.raw_items(), expected);
    let state = session.state.lock().await;
    assert_eq!(
        state.known_persisted_history_cursor(),
        persisted_history_cursor(&persisted_prefix)
    );
    assert_eq!(state.persisted_history_cursor_uncertainty(), None);
}

#[tokio::test]
async fn uncertain_append_proven_by_cursor_allows_persisted_rollback_suffix() {
    let (session, _turn_context) = make_session_and_context().await;
    let first_turn = completed_turn("turn-1", "first user", "first assistant");
    let second_turn = completed_turn("turn-2", "second user", "second assistant");
    let first_history = model_history_for_turn("first user", "first assistant");
    let mut local_history = first_history.clone();
    local_history.extend(model_history_for_turn("second user", "second assistant"));
    session.replace_history(local_history, None).await;
    set_known_persisted_history(&session, &first_turn).await;
    invalidate_persisted_history_cursor(&session, &second_turn).await;

    let rollback = rollback(/*num_turns*/ 1);
    let mut landed_rollout = first_turn;
    landed_rollout.extend(second_turn);
    landed_rollout.push(rollback);
    let outcome = reconcile_idle(&session, &landed_rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    assert_eq!(session.clone_history().await.raw_items(), first_history);
    let state = session.state.lock().await;
    assert_eq!(state.persisted_history_cursor_uncertainty(), None);
    assert_eq!(
        state.known_persisted_history_cursor(),
        persisted_history_cursor(&landed_rollout)
    );
}

#[tokio::test]
async fn uncertain_history_rewrite_never_restores_pre_rollback_disk_history() {
    let (session, _turn_context) = make_session_and_context().await;
    let first_turn = completed_turn("turn-1", "first user", "first assistant");
    let second_turn = completed_turn("turn-2", "second user", "second assistant");
    let mut pre_rollback_rollout = first_turn.clone();
    pre_rollback_rollout.extend(second_turn);
    let first_history = model_history_for_turn("first user", "first assistant");
    session.replace_history(first_history.clone(), None).await;
    set_known_persisted_history(&session, &pre_rollback_rollout).await;
    let rollback = rollback(/*num_turns*/ 1);
    invalidate_persisted_history_cursor(&session, std::slice::from_ref(&rollback)).await;

    let stale_outcome = reconcile_idle(&session, &pre_rollback_rollout).await;

    assert_eq!(stale_outcome, ThreadHistoryReconciliationOutcome::Conflict);
    assert_eq!(session.clone_history().await.raw_items(), first_history);
    assert_eq!(
        session
            .state
            .lock()
            .await
            .persisted_history_cursor_uncertainty(),
        Some(PersistedHistoryCursorUncertainty::HistoryRewrite)
    );

    let mut landed_rollout = pre_rollback_rollout;
    landed_rollout.push(rollback);
    let landed_outcome = reconcile_idle(&session, &landed_rollout).await;

    assert_eq!(
        landed_outcome,
        ThreadHistoryReconciliationOutcome::Unchanged
    );
    assert_eq!(session.clone_history().await.raw_items(), first_history);
    let state = session.state.lock().await;
    assert_eq!(state.persisted_history_cursor_uncertainty(), None);
    assert_eq!(
        state.known_persisted_history_cursor(),
        persisted_history_cursor(&landed_rollout)
    );
}

#[tokio::test]
async fn uncertain_event_only_rollback_requires_the_durable_marker() {
    let (session, _turn_context) = make_session_and_context().await;
    let mut pre_rollback_rollout = vec![turn_started("event-only-turn")];
    pre_rollback_rollout.push(turn_complete("event-only-turn"));
    session.replace_history(Vec::new(), None).await;
    set_known_persisted_history(&session, &pre_rollback_rollout).await;
    let rollback = rollback(/*num_turns*/ 1);
    invalidate_persisted_history_cursor(&session, std::slice::from_ref(&rollback)).await;

    let stale_outcome = reconcile_idle(&session, &pre_rollback_rollout).await;

    assert_eq!(stale_outcome, ThreadHistoryReconciliationOutcome::Conflict);
    assert!(session.clone_history().await.raw_items().is_empty());
    let mut expected_landed_rollout = pre_rollback_rollout.clone();
    expected_landed_rollout.push(rollback.clone());
    {
        let state = session.state.lock().await;
        assert_eq!(
            state.persisted_history_cursor_uncertainty(),
            Some(PersistedHistoryCursorUncertainty::HistoryRewrite)
        );
        assert_eq!(
            state.uncertain_expected_persisted_history_cursor(),
            persisted_history_cursor(&expected_landed_rollout)
        );
    }

    let landed_outcome = reconcile_idle(&session, &expected_landed_rollout).await;

    assert_eq!(
        landed_outcome,
        ThreadHistoryReconciliationOutcome::Unchanged
    );
    assert!(session.clone_history().await.raw_items().is_empty());
    let state = session.state.lock().await;
    assert_eq!(state.persisted_history_cursor_uncertainty(), None);
    assert_eq!(
        state.known_persisted_history_cursor(),
        persisted_history_cursor(&expected_landed_rollout)
    );
}
