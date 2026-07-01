use super::*;
use codex_protocol::items::HookPromptFragment;
use codex_protocol::items::build_hook_prompt_message;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn reconcile_persisted_history_hydrates_tokens_without_rewriting_equal_history() {
    let (session, turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("user", "assistant");
    session.replace_history(history.clone(), None).await;
    let old_info = token_usage_info(10);
    let new_info = token_usage_info(25);
    {
        let mut state = session.state.lock().await;
        state.set_token_info(Some(old_info));
    }
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    let before_version = session.state.lock().await.history.history_version();
    let mut rollout = completed_turn("turn-1", "user", "assistant");
    let mut persisted_turn_context = turn_context.to_turn_context_item();
    persisted_turn_context.turn_id = Some("turn-1".to_string());
    rollout.insert(3, RolloutItem::TurnContext(persisted_turn_context.clone()));
    rollout.push(RolloutItem::EventMsg(EventMsg::TokenCount(
        TokenCountEvent {
            info: Some(new_info.clone()),
            rate_limits: None,
        },
    )));

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    let state = session.state.lock().await;
    assert_eq!(state.history.raw_items(), history);
    assert_eq!(state.history.history_version(), before_version);
    assert_eq!(state.token_info(), Some(new_info));
    assert_eq!(
        state.previous_turn_settings(),
        Some(PreviousTurnSettings {
            model: persisted_turn_context.model.clone(),
            comp_hash: persisted_turn_context.comp_hash.clone(),
            realtime_active: persisted_turn_context.realtime_active,
        })
    );
    assert_eq!(state.reference_context_item(), Some(persisted_turn_context));
}

#[tokio::test]
async fn reconcile_persisted_history_prefers_busy_over_incomplete_tail() {
    let (session, _turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("local user", "local assistant");
    session.replace_history(history.clone(), None).await;
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    *session.active_turn.lock().await = Some(ActiveTurn::default());

    let outcome = session
        .reconcile_persisted_history(snapshot, &[turn_started("running-turn")])
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Busy);
    assert!(session.history_reconciliation_snapshot().await.is_none());
    assert_eq!(session.clone_history().await.raw_items(), history);
    *session.active_turn.lock().await = None;
}

#[tokio::test]
async fn reconcile_persisted_history_rejects_incomplete_idle_tail_without_mutation() {
    let (session, _turn_context) = make_session_and_context().await;
    let local_history = model_history_for_turn("local user", "local assistant");
    session.replace_history(local_history.clone(), None).await;
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    let mut rollout = vec![
        turn_started("external-turn"),
        RolloutItem::ResponseItem(user_message("external user")),
        RolloutItem::ResponseItem(assistant_message("partial assistant")),
    ];

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Incomplete);
    assert_eq!(session.clone_history().await.raw_items(), local_history);

    rollout.push(RolloutItem::EventMsg(EventMsg::TurnAborted(
        TurnAbortedEvent {
            turn_id: Some("external-turn".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        },
    )));
    let retry_snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle retry snapshot");
    let retry_outcome = session
        .reconcile_persisted_history(retry_snapshot, &rollout)
        .await;
    assert_eq!(retry_outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    assert_eq!(
        session.clone_history().await.raw_items(),
        model_history_for_turn("external user", "partial assistant")
    );
}

#[tokio::test]
async fn reconcile_persisted_history_requires_terminal_event_after_turn_error() {
    let (session, _turn_context) = make_session_and_context().await;
    let local_history = model_history_for_turn("local user", "local assistant");
    session.replace_history(local_history.clone(), None).await;
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    let rollout = vec![
        turn_started("failed-turn"),
        RolloutItem::ResponseItem(user_message("external user")),
        RolloutItem::EventMsg(EventMsg::Error(ErrorEvent {
            message: "external failure".to_string(),
            codex_error_info: None,
        })),
    ];

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Incomplete);
    assert_eq!(session.clone_history().await.raw_items(), local_history);
}

#[tokio::test]
async fn reconcile_persisted_history_allows_unchanged_known_crash_tail() {
    let (session, _turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("crash user", "partial assistant");
    let rollout = vec![
        turn_started("crash-turn"),
        RolloutItem::ResponseItem(user_message("crash user")),
        RolloutItem::ResponseItem(assistant_message("partial assistant")),
    ];
    session.replace_history(history.clone(), None).await;
    session
        .state
        .lock()
        .await
        .set_known_persisted_incomplete_tail(Some("crash-turn".to_string()));
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    assert_eq!(session.clone_history().await.raw_items(), history);
}

#[tokio::test]
async fn reconcile_persisted_history_allows_metadata_after_known_crash_tail() {
    let (session, _turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("crash user", "partial assistant");
    let mut rollout = vec![
        turn_started("crash-turn"),
        RolloutItem::ResponseItem(user_message("crash user")),
        RolloutItem::ResponseItem(assistant_message("partial assistant")),
    ];
    session.replace_history(history.clone(), None).await;
    session
        .state
        .lock()
        .await
        .set_known_persisted_incomplete_tail(Some("crash-turn".to_string()));
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    let thread_id = session.thread_id();
    rollout.push(RolloutItem::EventMsg(EventMsg::ThreadGoalUpdated(
        ThreadGoalUpdatedEvent {
            thread_id,
            turn_id: None,
            goal: ThreadGoal {
                thread_id,
                objective: "metadata only".to_string(),
                status: ThreadGoalStatus::Active,
                token_budget: None,
                tokens_used: 0,
                time_used_seconds: 0,
                created_at: 1,
                updated_at: 1,
            },
        },
    )));

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Unchanged);
    assert_eq!(session.clone_history().await.raw_items(), history);
}

#[tokio::test]
async fn reconcile_persisted_history_allows_rollback_after_known_crash_tail() {
    let (session, _turn_context) = make_session_and_context().await;
    let history = model_history_for_turn("crash user", "partial assistant");
    let mut rollout = vec![
        turn_started("crash-turn"),
        RolloutItem::ResponseItem(user_message("crash user")),
        RolloutItem::ResponseItem(assistant_message("partial assistant")),
    ];
    session.replace_history(history, None).await;
    session
        .state
        .lock()
        .await
        .set_known_persisted_incomplete_tail(Some("crash-turn".to_string()));
    let snapshot = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    rollout.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
        ThreadRolledBackEvent { num_turns: 1 },
    )));

    let outcome = session
        .reconcile_persisted_history(snapshot, &rollout)
        .await;

    assert_eq!(outcome, ThreadHistoryReconciliationOutcome::Refreshed);
    assert_eq!(session.clone_history().await.raw_items(), Vec::new());
    assert_eq!(
        session.state.lock().await.known_persisted_incomplete_tail(),
        None
    );
}

#[tokio::test]
async fn reconcile_persisted_history_detects_optimistic_conflicts() {
    let (session, turn_context) = make_session_and_context().await;
    session
        .replace_history(vec![user_message("local baseline")], None)
        .await;
    let snapshot_before_append = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    let local_append = assistant_message("local append");
    {
        let mut state = session.state.lock().await;
        state.history.record_items(
            std::iter::once(&local_append),
            turn_context.model_info.truncation_policy.into(),
        );
    }
    let persisted = completed_turn("external-turn", "external user", "external assistant");

    let append_outcome = session
        .reconcile_persisted_history(snapshot_before_append, &persisted)
        .await;

    assert_eq!(append_outcome, ThreadHistoryReconciliationOutcome::Conflict);
    let history_after_append = session.clone_history().await.raw_items().to_vec();
    assert!(history_after_append.contains(&local_append));

    let snapshot_before_rewrite = session
        .history_reconciliation_snapshot()
        .await
        .expect("idle history snapshot");
    session
        .replace_history(history_after_append.clone(), None)
        .await;
    let rewrite_outcome = session
        .reconcile_persisted_history(snapshot_before_rewrite, &persisted)
        .await;
    assert_eq!(
        rewrite_outcome,
        ThreadHistoryReconciliationOutcome::Conflict
    );
    assert_eq!(
        session.clone_history().await.raw_items(),
        history_after_append
    );
}

#[tokio::test]
async fn history_reconciliation_lock_serializes_idle_injection() {
    let (session, _turn_context) = make_session_and_context().await;
    let history_guard = session.acquire_history_persistence_lock().await;
    let injection = session.inject_no_new_turn(
        vec![assistant_message("injected while resume is pending")],
        None,
    );
    tokio::pin!(injection);

    assert!(
        timeout(Duration::from_millis(10), injection.as_mut())
            .await
            .is_err()
    );
    drop(history_guard);
    injection.await;
    let history = session.clone_history().await;
    assert!(
        matches!(
            history.raw_items(),
            [ResponseItem::Message { content, .. }]
                if matches!(content.as_slice(), [ContentItem::OutputText { text }]
                    if text == "injected while resume is pending")
        ),
        "unexpected injected history: {:?}",
        history.raw_items()
    );
}

#[tokio::test]
async fn history_reconciliation_event_cut_orders_pre_cut_during_and_post_cut_events() {
    let (session, _turn_context, rx_event) = make_session_and_context_with_rx().await;
    let pre_cut = Event {
        id: "pre-cut".to_string(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "pre-cut".to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        }),
    };
    session.send_event_raw(pre_cut.clone()).await;

    let event_cut_guards = session.acquire_history_reconciliation_event_cut().await;
    let received_pre_cut = rx_event.try_recv().expect("pre-cut event should be queued");
    assert_eq!(received_pre_cut.id, pre_cut.id);
    assert!(matches!(received_pre_cut.msg, EventMsg::TurnStarted(_)));

    let during_cut = Event {
        id: "during-cut".to_string(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "during-cut".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    };
    let session_for_during_cut = Arc::clone(&session);
    let during_cut_for_task = during_cut.clone();
    let mut during_cut_task = tokio::spawn(async move {
        session_for_during_cut
            .send_event_raw(during_cut_for_task)
            .await;
    });
    assert!(
        timeout(Duration::from_millis(25), &mut during_cut_task)
            .await
            .is_err(),
        "an event arriving during the cut must wait until after the snapshot"
    );
    assert!(matches!(
        rx_event.try_recv(),
        Err(async_channel::TryRecvError::Empty)
    ));

    drop(event_cut_guards);
    timeout(Duration::from_secs(1), during_cut_task)
        .await
        .expect("during-cut event should unblock")
        .expect("during-cut event task should not panic");
    let received_during_cut = rx_event
        .try_recv()
        .expect("during-cut event should be delivered after release");
    assert_eq!(received_during_cut.id, during_cut.id);
    assert!(matches!(received_during_cut.msg, EventMsg::TurnComplete(_)));

    let post_cut = Event {
        id: "post-cut".to_string(),
        msg: EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some("post-cut".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    };
    session.send_event_raw(post_cut.clone()).await;
    let received_post_cut = rx_event
        .try_recv()
        .expect("post-cut event should be delivered");
    assert_eq!(received_post_cut.id, post_cut.id);
    assert!(matches!(received_post_cut.msg, EventMsg::TurnAborted(_)));
    assert!(matches!(
        rx_event.try_recv(),
        Err(async_channel::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn history_reconciliation_event_cut_keeps_hook_prompt_persistence_with_raw_event() {
    let (session, turn_context, rx_event) = make_session_and_context_with_rx().await;
    let hook_prompt = build_hook_prompt_message(&[HookPromptFragment::from_single_hook(
        "continue after the stop hook",
        "hook-run-1",
    )])
    .expect("hook prompt message");
    let rollout_guard = session.acquire_rollout_persistence_lock().await;
    let session_for_record = Arc::clone(&session);
    let turn_context_for_record = Arc::clone(&turn_context);
    let mut record_task = tokio::spawn(async move {
        session_for_record
            .record_conversation_items(
                turn_context_for_record.as_ref(),
                std::slice::from_ref(&hook_prompt),
            )
            .await;
    });

    timeout(Duration::from_secs(1), async {
        loop {
            if !session.clone_history().await.raw_items().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("hook prompt should reach in-memory history before persistence unblocks");

    let session_for_cut = Arc::clone(&session);
    let (cut_queued_tx, cut_queued_rx) = tokio::sync::oneshot::channel();
    let (cut_acquired_tx, mut cut_acquired_rx) = tokio::sync::oneshot::channel();
    let (release_cut_tx, release_cut_rx) = tokio::sync::oneshot::channel();
    let cut_task = tokio::spawn(async move {
        let history_guard = session_for_cut.acquire_history_persistence_lock().await;
        let _ = cut_queued_tx.send(());
        let event_guard = session_for_cut.acquire_event_delivery_lock().await;
        let rollout_guard = session_for_cut.acquire_rollout_persistence_lock().await;
        let _ = cut_acquired_tx.send(());
        let _ = release_cut_rx.await;
        drop((history_guard, event_guard, rollout_guard));
    });
    timeout(Duration::from_secs(1), cut_queued_rx)
        .await
        .expect("the cut should queue behind the event transaction")
        .expect("cut task should report that its history guard is held");
    assert!(
        timeout(Duration::from_millis(25), &mut cut_acquired_rx)
            .await
            .is_err(),
        "the cut must wait while the hook prompt batch is blocked on persistence"
    );

    drop(rollout_guard);
    timeout(Duration::from_secs(1), &mut cut_acquired_rx)
        .await
        .expect("the cut should acquire after the hook prompt batch")
        .expect("cut task should report acquisition");
    let raw_event_precedes_cut = std::iter::from_fn(|| rx_event.try_recv().ok())
        .any(|event| matches!(event.msg, EventMsg::RawResponseItem(_)));
    let _ = release_cut_tx.send(());
    timeout(Duration::from_secs(1), cut_task)
        .await
        .expect("cut task should finish")
        .expect("cut task should not panic");
    timeout(Duration::from_secs(1), &mut record_task)
        .await
        .expect("hook prompt record should finish")
        .expect("hook prompt record task should not panic");

    assert!(
        raw_event_precedes_cut,
        "the raw hook-prompt event must be delivered before a resume snapshot can include its persisted response item"
    );
}

#[tokio::test]
async fn history_reconciliation_event_cut_keeps_record_and_item_lifecycle_in_one_batch() {
    let (session, turn_context, rx_event) = make_session_and_context_with_rx().await;
    let response_item = assistant_message("batched assistant item");
    let rollout_guard = session.acquire_rollout_persistence_lock().await;
    let session_for_record = Arc::clone(&session);
    let turn_context_for_record = Arc::clone(&turn_context);
    let mut record_task = tokio::spawn(async move {
        session_for_record
            .record_response_item_and_emit_turn_item(
                turn_context_for_record.as_ref(),
                response_item,
            )
            .await;
    });

    timeout(Duration::from_secs(1), async {
        loop {
            if !session.clone_history().await.raw_items().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("assistant item should reach in-memory history before persistence unblocks");

    let session_for_cut = Arc::clone(&session);
    let (cut_queued_tx, cut_queued_rx) = tokio::sync::oneshot::channel();
    let (cut_acquired_tx, mut cut_acquired_rx) = tokio::sync::oneshot::channel();
    let (release_cut_tx, release_cut_rx) = tokio::sync::oneshot::channel();
    let cut_task = tokio::spawn(async move {
        let history_guard = session_for_cut.acquire_history_persistence_lock().await;
        let _ = cut_queued_tx.send(());
        let event_guard = session_for_cut.acquire_event_delivery_lock().await;
        let rollout_guard = session_for_cut.acquire_rollout_persistence_lock().await;
        let _ = cut_acquired_tx.send(());
        let _ = release_cut_rx.await;
        drop((history_guard, event_guard, rollout_guard));
    });
    timeout(Duration::from_secs(1), cut_queued_rx)
        .await
        .expect("the cut should queue behind the event transaction")
        .expect("cut task should report that its history guard is held");
    assert!(
        timeout(Duration::from_millis(25), &mut cut_acquired_rx)
            .await
            .is_err(),
        "the cut must wait while the item batch is blocked on persistence"
    );

    drop(rollout_guard);
    timeout(Duration::from_secs(1), &mut cut_acquired_rx)
        .await
        .expect("the cut should acquire after the item batch")
        .expect("cut task should report acquisition");
    let events: Vec<EventMsg> = std::iter::from_fn(|| rx_event.try_recv().ok())
        .map(|event| event.msg)
        .collect();
    let raw_event_precedes_cut = events
        .iter()
        .any(|event| matches!(event, EventMsg::RawResponseItem(_)));
    let item_started_precedes_cut = events
        .iter()
        .any(|event| matches!(event, EventMsg::ItemStarted(_)));
    let item_completed_precedes_cut = events
        .iter()
        .any(|event| matches!(event, EventMsg::ItemCompleted(_)));
    let _ = release_cut_tx.send(());
    timeout(Duration::from_secs(1), cut_task)
        .await
        .expect("cut task should finish")
        .expect("cut task should not panic");
    timeout(Duration::from_secs(1), &mut record_task)
        .await
        .expect("assistant item record should finish")
        .expect("assistant item record task should not panic");

    assert!(raw_event_precedes_cut, "raw response item must precede cut");
    assert!(item_started_precedes_cut, "item/started must precede cut");
    assert!(
        item_completed_precedes_cut,
        "item/completed must precede cut"
    );
}

#[tokio::test]
async fn history_reconciliation_locks_serialize_append_and_cursor_update() {
    let (session, _turn_context) = make_session_and_context().await;
    let reconciliation_guards = session.acquire_history_reconciliation_event_cut().await;
    let rollout_items = [RolloutItem::ResponseItem(assistant_message(
        "append waiting behind reconciliation",
    ))];
    let append = session.persist_rollout_items(&rollout_items);
    tokio::pin!(append);

    assert!(
        timeout(Duration::from_millis(10), append.as_mut())
            .await
            .is_err()
    );
    drop(reconciliation_guards);
    append.await;
}
