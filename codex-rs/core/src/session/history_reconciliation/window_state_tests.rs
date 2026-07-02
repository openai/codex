use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn reconcile_persisted_history_installs_compaction_checkpoint() {
    let (session, _turn_context) = make_session_and_context().await;
    session
        .replace_history(model_history_for_turn("old user", "old assistant"), None)
        .await;
    {
        let mut state = session.state.lock().await;
        state.request_new_context_window();
        assert!(state.claim_token_budget_reminder());
        assert!(!state.claim_token_budget_reminder());
    }
    let compacted_history = model_history_for_turn("summary", "after summary");
    let window_ids = new_compaction_window_ids();
    let rollout = vec![
        turn_started("compact-turn"),
        compacted_item(
            compacted_history.clone(),
            /*window_number*/ 7,
            window_ids,
        ),
        turn_complete("compact-turn"),
    ];

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    let mut state = session.state.lock().await;
    assert_eq!(state.history.raw_items(), compacted_history);
    assert_eq!(state.auto_compact_window_number(), 7);
    assert_eq!(state.auto_compact_window_ids(), window_ids);
    assert!(!state.take_new_context_window_request());
    assert!(state.claim_token_budget_reminder());
}

#[tokio::test]
async fn reconcile_persisted_history_resets_runtime_state_for_new_compaction_window() {
    let (session, _turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("summary", "after summary");
    session.replace_history(history.clone(), None).await;
    {
        let mut state = session.state.lock().await;
        state.ensure_auto_compact_window_server_prefill_from_usage(&TokenUsage {
            input_tokens: 777,
            total_tokens: 777,
            ..Default::default()
        });
        state.request_new_context_window();
        assert!(state.claim_token_budget_reminder());
    }
    let window_ids = new_compaction_window_ids();
    let rollout = vec![
        turn_started("compact-turn"),
        compacted_item(history.clone(), /*window_number*/ 7, window_ids),
        turn_complete("compact-turn"),
    ];

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    let mut state = session.state.lock().await;
    assert_ne!(
        state.auto_compact_window_snapshot().prefill_input_tokens,
        Some(777)
    );
    assert!(!state.take_new_context_window_request());
    assert!(state.claim_token_budget_reminder());
}

#[tokio::test]
async fn reconcile_persisted_history_queues_only_unconsumed_imported_compaction_hook() {
    for followed_by_turn in [false, true] {
        let (session, _turn_context) = make_session_and_context().await;
        let first_history = model_history_for_turn("first user", "first assistant");
        session.replace_history(first_history, None).await;
        let mut rollout = completed_turn("turn-1", "first user", "first assistant");
        set_known_persisted_history(&session, &rollout).await;
        {
            let mut state = session.state.lock().await;
            while state.take_pending_session_start_source().is_some() {}
        }
        let window_ids = new_compaction_window_ids();
        rollout.extend([
            turn_started("compact-turn"),
            compacted_item(
                model_history_for_turn("summary", "after summary"),
                /*window_number*/ 7,
                window_ids,
            ),
            turn_complete("compact-turn"),
        ]);
        if followed_by_turn {
            rollout.extend(completed_turn(
                "turn-after-compact",
                "later user",
                "later assistant",
            ));
        }

        let outcome = reconcile_idle(&session, &rollout).await;

        assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
        let pending_source = session
            .state
            .lock()
            .await
            .take_pending_session_start_source();
        if followed_by_turn {
            assert!(
                pending_source.is_none(),
                "a later turn already consumed the imported compact lifecycle hook"
            );
        } else {
            assert!(
                matches!(
                    pending_source,
                    Some(codex_hooks::SessionStartSource::Compact)
                ),
                "an imported compaction must queue its next-turn lifecycle hook"
            );
        }
    }
}

#[tokio::test]
async fn cursor_mismatch_queues_imported_compaction_hook_exactly_once() {
    let (session, _turn_context) = make_session_and_context().await;
    let first_history = model_history_for_turn("first user", "first assistant");
    let local_item = assistant_message("locally known item after old prefix");
    let mut local_history = first_history.clone();
    local_history.push(local_item.clone());
    session.replace_history(local_history.clone(), None).await;

    let first_turn = completed_turn("turn-1", "first user", "first assistant");
    let mut locally_known_rollout = first_turn.clone();
    locally_known_rollout.push(RolloutItem::ResponseItem(local_item));
    set_known_persisted_history(&session, &locally_known_rollout).await;
    {
        let mut state = session.state.lock().await;
        while state.take_pending_session_start_source().is_some() {}
    }

    let window_ids = new_compaction_window_ids();
    let mut actual_rollout = first_turn;
    actual_rollout.extend([
        turn_started("compact-turn"),
        compacted_item(local_history, /*window_number*/ 7, window_ids),
        turn_complete("compact-turn"),
    ]);

    let outcome = reconcile_idle(&session, &actual_rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    assert!(matches!(
        session
            .state
            .lock()
            .await
            .take_pending_session_start_source(),
        Some(codex_hooks::SessionStartSource::Compact)
    ));

    let second_outcome = reconcile_idle(&session, &actual_rollout).await;

    assert_eq!(
        second_outcome,
        ThreadHistoryReconciliationOutcome::Unchanged
    );
    assert!(
        session
            .state
            .lock()
            .await
            .take_pending_session_start_source()
            .is_none(),
        "the installed full cursor must prevent duplicate Compact lifecycle hooks"
    );
}

#[tokio::test]
async fn reconcile_persisted_history_applies_external_rollback() {
    let (session, _turn_context) = make_session_and_context().await;
    let first_history = model_history_for_turn("first user", "first assistant");
    let second_history = model_history_for_turn("second user", "second assistant");
    let mut local_history = first_history.clone();
    local_history.extend(second_history);
    session.replace_history(local_history, None).await;
    let window_id = session
        .state
        .lock()
        .await
        .auto_compact_window_ids()
        .window_id
        .to_string();
    let budget = session.services.agent_control.rollout_budget();
    budget.configure(RolloutBudgetConfig {
        limit_tokens: 100,
        reminder_at_remaining_tokens: vec![100],
        sampling_token_weight: 1.0,
        prefill_token_weight: 1.0,
    });
    let reminder = budget
        .pending_reminder(session.thread_id(), &window_id)
        .expect("initial rollout budget reminder");
    budget.mark_reminder_delivered(session.thread_id(), &window_id, reminder);
    assert!(
        budget
            .pending_reminder(session.thread_id(), &window_id)
            .is_none()
    );
    let mut additional_context = BTreeMap::new();
    additional_context.insert(
        "project".to_string(),
        AdditionalContextEntry {
            value: "keep this available after rollback".to_string(),
            kind: AdditionalContextKind::Application,
        },
    );
    let mut rollout = completed_turn("turn-1", "first user", "first assistant");
    rollout.extend(completed_turn("turn-2", "second user", "second assistant"));
    set_known_persisted_history(&session, &rollout).await;
    {
        let mut state = session.state.lock().await;
        let _ = state.additional_context.merge(additional_context.clone());
    }
    rollout.push(rollback(/*num_turns*/ 1));

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    assert_eq!(session.clone_history().await.raw_items(), first_history);
    assert!(
        budget
            .pending_reminder(session.thread_id(), &window_id)
            .is_some()
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .additional_context
            .merge(additional_context)
            .len(),
        1,
        "an imported rollback must rearm additional-context emission"
    );
}

#[tokio::test]
async fn reconcile_persisted_history_rearms_token_budget_reminder_removed_by_rollback() {
    for scope in [
        AutoCompactTokenLimitScope::Total,
        AutoCompactTokenLimitScope::BodyAfterPrefix,
    ] {
        let (session, mut turn_context) = make_session_and_context().await;
        let reminder_template = "reconciled budget reminder: {n_remaining}";
        let mut config = (*turn_context.config).clone();
        config.token_budget = Some(TokenBudgetConfig {
            reminder_threshold_tokens: Some(50),
            reminder_message_template: reminder_template.to_string(),
            guidance_message: None,
        });
        config.model_auto_compact_token_limit_scope = scope;
        config
            .features
            .enable(Feature::TokenBudget)
            .expect("test config should allow token budget");
        turn_context.config = Arc::new(config);
        turn_context.sub_id = "turn-2".to_string();

        let first_history = model_history_for_turn("first user", "first assistant");
        let second_history = model_history_for_turn("second user", "second assistant");
        let mut local_history = first_history.clone();
        local_history.extend(second_history.clone());
        session.replace_history(local_history, None).await;

        super::token_budget::maybe_record(&session, &turn_context, Some(25)).await;

        let mut delivered_reminder = ContextualUserFragment::into(TokenBudgetReminder::new(
            reminder_template,
            /*n_remaining*/ 25,
        ));
        delivered_reminder.set_turn_id_if_missing(&turn_context.sub_id);
        let mut expected_delivered_history = first_history.clone();
        expected_delivered_history.extend(second_history);
        expected_delivered_history.push(delivered_reminder.clone());
        assert_eq!(
            session.clone_history().await.raw_items(),
            expected_delivered_history
        );

        let mut rollout = completed_turn("turn-1", "first user", "first assistant");
        let mut second_turn = completed_turn("turn-2", "second user", "second assistant");
        second_turn.insert(
            second_turn.len() - 1,
            RolloutItem::ResponseItem(delivered_reminder.clone()),
        );
        rollout.extend(second_turn);
        let (window_number, window_ids) = {
            let mut state = session.state.lock().await;
            let mut config = (*state.session_configuration.original_config_do_not_use).clone();
            config.model_auto_compact_token_limit_scope = scope;
            state.session_configuration.original_config_do_not_use = Arc::new(config);
            state.set_known_persisted_history_cursor(persisted_history_cursor(&rollout));
            state.ensure_auto_compact_window_server_prefill_from_usage(&TokenUsage {
                input_tokens: 777,
                total_tokens: 777,
                ..Default::default()
            });
            assert!(!state.claim_token_budget_reminder());
            (
                state.auto_compact_window_number(),
                state.auto_compact_window_ids(),
            )
        };
        rollout.push(rollback(/*num_turns*/ 1));

        let outcome = reconcile_idle(&session, &rollout).await;

        assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
        assert_eq!(session.clone_history().await.raw_items(), first_history);
        {
            let state = session.state.lock().await;
            assert_eq!(state.auto_compact_window_number(), window_number);
            assert_eq!(state.auto_compact_window_ids(), window_ids);
            assert_eq!(
                state.auto_compact_window_snapshot().prefill_input_tokens,
                Some(777),
                "same-window rollback must preserve the server prefill for {scope:?}"
            );
        }

        super::token_budget::maybe_record(&session, &turn_context, Some(25)).await;

        let mut expected_history = first_history;
        expected_history.push(delivered_reminder);
        assert_eq!(session.clone_history().await.raw_items(), expected_history);

        super::token_budget::maybe_record(&session, &turn_context, Some(25)).await;
        assert_eq!(
            session.clone_history().await.raw_items(),
            expected_history,
            "the re-emitted reminder remains one-shot within the reconciled window"
        );
    }
}

#[tokio::test]
async fn reconcile_persisted_history_preserves_loaded_prefix_truncation() {
    let (session, _turn_context) = make_session_and_context().await;
    let preserved_prefix = model_history_for_turn("truncated user", "truncated assistant");
    session
        .replace_history(preserved_prefix.clone(), None)
        .await;
    let mut additional_context = BTreeMap::new();
    additional_context.insert(
        "project".to_string(),
        AdditionalContextEntry {
            value: "primary context before external turn".to_string(),
            kind: AdditionalContextKind::Application,
        },
    );
    let _ = session
        .state
        .lock()
        .await
        .additional_context
        .merge(additional_context.clone());
    let mut rollout = completed_turn("turn-1", "raw user", "raw assistant");
    set_known_persisted_history(&session, &rollout).await;
    set_server_prefill(&session, /*input_tokens*/ 777).await;
    rollout.extend(completed_turn(
        "turn-2",
        "external user",
        "external assistant",
    ));

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    let mut expected = preserved_prefix;
    expected.extend(model_history_for_turn(
        "external user",
        "external assistant",
    ));
    assert_eq!(session.clone_history().await.raw_items(), expected);
    assert_eq!(
        server_prefill(&session).await,
        Some(777),
        "suffix reconciliation must preserve the prefill after restoring the loaded prefix"
    );
    assert_eq!(
        session.state.lock().await.known_persisted_history_cursor(),
        persisted_history_cursor(&rollout)
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .additional_context
            .merge(additional_context)
            .len(),
        1,
        "an imported turn must rearm additional-context emission"
    );
}

#[tokio::test]
async fn reconcile_persisted_history_preserves_same_window_prefill_for_append_only_suffix() {
    let (session, _turn_context) = make_session_and_context().await;
    let local_history = model_history_for_turn("first user", "first assistant");
    session.replace_history(local_history, None).await;
    let mut rollout = completed_turn("turn-1", "first user", "first assistant");
    set_known_persisted_history(&session, &rollout).await;
    set_server_prefill(&session, /*input_tokens*/ 777).await;
    rollout.extend(completed_turn(
        "turn-2",
        "external user",
        "external assistant",
    ));

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    assert_eq!(
        server_prefill(&session).await,
        Some(777),
        "append-only reconciliation in the same compaction window must retain the server baseline"
    );
}

#[tokio::test]
async fn reconcile_persisted_history_ignores_interleaved_session_metadata_in_cursor() {
    let (session, _turn_context) = make_session_and_context().await;
    let mut preserved_prefix = model_history_for_turn("first user", "first assistant");
    let local_item = assistant_message("local item after metadata update");
    preserved_prefix.push(local_item.clone());
    session
        .replace_history(preserved_prefix.clone(), None)
        .await;

    let mut rollout = completed_turn("turn-1", "first user", "first assistant");
    rollout.push(session_meta(session.thread_id()));
    rollout.push(RolloutItem::ResponseItem(local_item));
    set_known_persisted_history(&session, &rollout).await;
    rollout.extend(completed_turn(
        "turn-2",
        "external user",
        "external assistant",
    ));

    let outcome = reconcile_idle(&session, &rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    let mut expected = preserved_prefix;
    expected.extend(model_history_for_turn(
        "external user",
        "external assistant",
    ));
    assert_eq!(session.clone_history().await.raw_items(), expected);
    assert_eq!(
        session.state.lock().await.known_persisted_history_cursor(),
        persisted_history_cursor(&rollout)
    );
}

#[tokio::test]
async fn reconcile_persisted_history_rebuilds_after_external_append_interleaves_prefix() {
    let (session, _turn_context) = make_session_and_context().await;
    let local_item = assistant_message("local item after unseen external turn");
    let mut local_history = model_history_for_turn("first user", "first assistant");
    local_history.push(local_item.clone());
    session.replace_history(local_history, None).await;

    let first_turn = completed_turn("turn-1", "first user", "first assistant");
    let mut locally_known_rollout = first_turn.clone();
    locally_known_rollout.push(RolloutItem::ResponseItem(local_item.clone()));
    set_known_persisted_history(&session, &locally_known_rollout).await;

    let mut actual_rollout = first_turn;
    actual_rollout.extend(completed_turn(
        "turn-2",
        "external user",
        "external assistant",
    ));
    actual_rollout.push(RolloutItem::ResponseItem(local_item.clone()));

    let outcome = reconcile_idle(&session, &actual_rollout).await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    let mut expected = model_history_for_turn("first user", "first assistant");
    expected.extend(model_history_for_turn(
        "external user",
        "external assistant",
    ));
    expected.push(local_item);
    assert_eq!(session.clone_history().await.raw_items(), expected);
    assert_eq!(
        session.state.lock().await.known_persisted_history_cursor(),
        persisted_history_cursor(&actual_rollout)
    );
}
