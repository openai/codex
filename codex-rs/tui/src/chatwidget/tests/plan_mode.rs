use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn plan_implementation_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.on_plan_item_completed("- Step 1\n- Step 2\n".to_string());
    chat.open_plan_implementation_prompt();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("plan_implementation_popup", popup);
}

#[tokio::test]
async fn plan_implementation_popup_no_selected_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.on_plan_item_completed("- Step 1\n- Step 2\n".to_string());
    chat.open_plan_implementation_prompt();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("plan_implementation_popup_no_selected", popup);
}

#[tokio::test]
async fn plan_implementation_popup_yes_emits_submit_message_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.open_plan_implementation_prompt();

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected AppEvent");
    let AppEvent::SubmitUserMessageWithMode {
        text,
        collaboration_mode,
    } = event
    else {
        panic!("expected SubmitUserMessageWithMode, got {event:?}");
    };
    assert_eq!(
        text,
        plan_implementation::PLAN_IMPLEMENTATION_CODING_MESSAGE
    );
    assert_eq!(collaboration_mode.mode, Some(ModeKind::Default));
}

#[tokio::test]
async fn plan_implementation_popup_clear_context_emits_clear_submit_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    let plan_markdown = "- Step 1\n- Step 2\n";
    chat.on_plan_item_completed(plan_markdown.to_string());
    let _ = drain_insert_history(&mut rx);
    chat.open_plan_implementation_prompt();

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected AppEvent");
    let AppEvent::ClearUiAndSubmitUserMessage { text } = event else {
        panic!("expected ClearUiAndSubmitUserMessage, got {event:?}");
    };
    assert_eq!(
        text,
        "A previous agent produced the plan below to accomplish the user's task. \
        Implement the plan in a fresh context. Treat the plan as the source of \
        user intent, re-read files as needed, and carry the work through \
        implementation and verification.\n\n- Step 1\n- Step 2\n"
    );
}

#[tokio::test]
async fn plan_implementation_clear_context_requires_default_mode_and_plan() {
    let (chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    let default_mask = collaboration_modes::default_mode_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");

    let params =
        plan_implementation::selection_view_params(/*default_mask*/ None, Some("- Step\n"));
    assert_eq!(
        params.items[1].disabled_reason.as_deref(),
        Some(plan_implementation::PLAN_IMPLEMENTATION_DEFAULT_UNAVAILABLE)
    );

    let params = plan_implementation::selection_view_params(
        Some(default_mask.clone()),
        /*plan_markdown*/ None,
    );
    assert_eq!(
        params.items[1].disabled_reason.as_deref(),
        Some(plan_implementation::PLAN_IMPLEMENTATION_NO_APPROVED_PLAN)
    );

    let params =
        plan_implementation::selection_view_params(Some(default_mask.clone()), Some("  \n"));
    assert_eq!(
        params.items[1].disabled_reason.as_deref(),
        Some(plan_implementation::PLAN_IMPLEMENTATION_NO_APPROVED_PLAN)
    );

    let params = plan_implementation::selection_view_params(Some(default_mask), Some("- Step\n"));
    assert_eq!(params.items[1].disabled_reason, None);
    assert!(!params.items[1].actions.is_empty());
}

#[tokio::test]
async fn submit_user_message_with_mode_sets_coding_collaboration_mode() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    let default_mode = collaboration_modes::default_mode_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    chat.submit_user_message_with_mode("Implement the plan.".to_string(), default_mode);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: None,
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with default collab mode, got {other:?}")
        }
    }
}

#[tokio::test]
async fn reasoning_selection_in_plan_mode_opens_scope_prompt_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    let _ = drain_insert_history(&mut rx);
    set_chatgpt_auth(&mut chat);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected AppEvent");
    assert_matches!(
        event,
        AppEvent::OpenPlanReasoningScopePrompt {
            model,
            effort: Some(_)
        } if model == "gpt-5.1-codex-max"
    );
}

#[tokio::test]
async fn reasoning_selection_in_plan_mode_without_effort_change_does_not_open_scope_prompt_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    let _ = drain_insert_history(&mut rx);
    set_chatgpt_auth(&mut chat);

    let current_preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.set_reasoning_effort(Some(current_preset.default_reasoning_effort));

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdateModel(model) if model == "gpt-5.1-codex-max"
        )),
        "expected model update event; events: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AppEvent::UpdateReasoningEffort(Some(_)))),
        "expected reasoning update event; events: {events:?}"
    );
}

#[tokio::test]
async fn reasoning_selection_in_plan_mode_matching_plan_effort_but_different_global_opens_scope_prompt()
 {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    let _ = drain_insert_history(&mut rx);
    set_chatgpt_auth(&mut chat);

    // Reproduce: Plan effective reasoning remains the preset (medium), but the
    // global default differs (high). Pressing Enter on the current Plan choice
    // should open the scope prompt rather than silently rewriting the global default.
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected AppEvent");
    assert_matches!(
        event,
        AppEvent::OpenPlanReasoningScopePrompt {
            model,
            effort: Some(ReasoningEffortConfig::Medium)
        } if model == "gpt-5.1-codex-max"
    );
}

#[tokio::test]
async fn plan_mode_reasoning_override_is_marked_current_in_reasoning_popup() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    set_chatgpt_auth(&mut chat);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::Low));

    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(popup.contains("Low (current)"));
    assert!(
        !popup.contains("High (current)"),
        "expected Plan override to drive current reasoning label, got: {popup}"
    );
}

#[tokio::test]
async fn reasoning_selection_in_plan_mode_model_switch_does_not_open_scope_prompt_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);
    let _ = drain_insert_history(&mut rx);
    set_chatgpt_auth(&mut chat);

    let preset = get_available_model(&chat, "gpt-5");
    chat.open_reasoning_popup(preset);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdateModel(model) if model == "gpt-5"
        )),
        "expected model update event; events: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AppEvent::UpdateReasoningEffort(Some(_)))),
        "expected reasoning update event; events: {events:?}"
    );
}

#[tokio::test]
async fn plan_reasoning_scope_popup_all_modes_persists_global_and_plan_override() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.open_plan_reasoning_scope_prompt(
        "gpt-5.1-codex-max".to_string(),
        Some(ReasoningEffortConfig::High),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdatePlanModeReasoningEffort(Some(ReasoningEffortConfig::High))
        )),
        "expected plan override to be updated; events: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::PersistPlanModeReasoningEffort(Some(ReasoningEffortConfig::High))
        )),
        "expected updated plan override to be persisted; events: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::PersistModelSelection { model, effort: Some(ReasoningEffortConfig::High) }
                if model == "gpt-5.1-codex-max"
        )),
        "expected global model reasoning selection persistence; events: {events:?}"
    );
}

#[test]
fn plan_mode_prompt_notification_uses_dedicated_type_name() {
    let notification = Notification::PlanModePrompt {
        title: PLAN_IMPLEMENTATION_TITLE.to_string(),
    };

    assert!(notification.allowed_for(&Notifications::Custom(
        vec!["plan-mode-prompt".to_string(),]
    )));
    assert!(!notification.allowed_for(&Notifications::Custom(vec![
        "approval-requested".to_string(),
    ])));
    assert_eq!(
        notification.display(),
        format!("Plan mode prompt: {PLAN_IMPLEMENTATION_TITLE}")
    );
}

#[test]
fn user_input_requested_notification_uses_dedicated_type_name() {
    let notification = Notification::UserInputRequested {
        question_count: 1,
        summary: Some("Reasoning scope".to_string()),
    };

    assert!(notification.allowed_for(&Notifications::Custom(vec![
        "user-input-requested".to_string(),
    ])));
    assert!(!notification.allowed_for(&Notifications::Custom(vec![
        "approval-requested".to_string(),
    ])));
    assert_eq!(
        notification.display(),
        "Question requested: Reasoning scope"
    );
}

#[tokio::test]
async fn open_plan_implementation_prompt_sets_pending_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.config.tui_notifications.notifications =
        Notifications::Custom(vec!["plan-mode-prompt".to_string()]);

    chat.open_plan_implementation_prompt();

    assert_matches!(
        chat.pending_notification,
        Some(Notification::PlanModePrompt { ref title }) if title == PLAN_IMPLEMENTATION_TITLE
    );
}

#[tokio::test]
async fn open_plan_reasoning_scope_prompt_sets_pending_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.config.tui_notifications.notifications =
        Notifications::Custom(vec!["plan-mode-prompt".to_string()]);

    chat.open_plan_reasoning_scope_prompt(
        "gpt-5.1-codex-max".to_string(),
        Some(ReasoningEffortConfig::High),
    );

    assert_matches!(
        chat.pending_notification,
        Some(Notification::PlanModePrompt { ref title }) if title == PLAN_MODE_REASONING_SCOPE_TITLE
    );
}

#[tokio::test]
async fn agent_turn_complete_does_not_override_pending_plan_mode_prompt_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;

    chat.open_plan_implementation_prompt();
    chat.notify(Notification::AgentTurnComplete {
        response: "done".to_string(),
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::PlanModePrompt { ref title }) if title == PLAN_IMPLEMENTATION_TITLE
    );
}

#[tokio::test]
async fn user_input_notification_overrides_pending_agent_turn_complete_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;

    chat.notify(Notification::AgentTurnComplete {
        response: "done".to_string(),
    });
    chat.handle_request_user_input_now(RequestUserInputEvent {
        call_id: "call-1".to_string(),
        turn_id: "turn-1".to_string(),
        questions: vec![RequestUserInputQuestion {
            id: "reasoning_scope".to_string(),
            header: "Reasoning scope".to_string(),
            question: "Which reasoning scope should I use?".to_string(),
            is_other: false,
            is_secret: false,
            options: Some(vec![RequestUserInputQuestionOption {
                label: "Plan only".to_string(),
                description: "Update only Plan mode.".to_string(),
            }]),
        }],
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::UserInputRequested {
            question_count: 1,
            summary: Some(ref summary),
        }) if summary == "Reasoning scope"
    );
}

#[tokio::test]
async fn handle_request_user_input_sets_pending_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.config.tui_notifications.notifications =
        Notifications::Custom(vec!["user-input-requested".to_string()]);

    chat.handle_request_user_input_now(RequestUserInputEvent {
        call_id: "call-1".to_string(),
        turn_id: "turn-1".to_string(),
        questions: vec![RequestUserInputQuestion {
            id: "reasoning_scope".to_string(),
            header: "Reasoning scope".to_string(),
            question: "Which reasoning scope should I use?".to_string(),
            is_other: false,
            is_secret: false,
            options: Some(vec![RequestUserInputQuestionOption {
                label: "Plan only".to_string(),
                description: "Update only Plan mode.".to_string(),
            }]),
        }],
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::UserInputRequested {
            question_count: 1,
            summary: Some(ref summary),
        }) if summary == "Reasoning scope"
    );
}

#[tokio::test]
async fn plan_reasoning_scope_popup_mentions_selected_reasoning() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::Low));
    chat.open_plan_reasoning_scope_prompt(
        "gpt-5.1-codex-max".to_string(),
        Some(ReasoningEffortConfig::Medium),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(popup.contains("Choose where to apply medium reasoning."));
    assert!(popup.contains("Always use medium reasoning in Plan mode."));
    assert!(popup.contains("Apply to Plan mode override"));
    assert!(popup.contains("Apply to global default and Plan mode override"));
    assert!(popup.contains("user-chosen Plan override (low)"));
}

#[tokio::test]
async fn plan_reasoning_scope_popup_mentions_built_in_plan_default_when_no_override() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.open_plan_reasoning_scope_prompt(
        "gpt-5.1-codex-max".to_string(),
        Some(ReasoningEffortConfig::Medium),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(popup.contains("built-in Plan default (medium)"));
}

#[tokio::test]
async fn plan_reasoning_scope_popup_plan_only_does_not_update_all_modes_reasoning() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.open_plan_reasoning_scope_prompt(
        "gpt-5.1-codex-max".to_string(),
        Some(ReasoningEffortConfig::High),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdatePlanModeReasoningEffort(Some(ReasoningEffortConfig::High))
        )),
        "expected plan-only reasoning update; events: {events:?}"
    );
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, AppEvent::UpdateReasoningEffort(_))),
        "did not expect all-modes reasoning update; events: {events:?}"
    );
}

#[tokio::test]
async fn submit_user_message_with_mode_errors_when_mode_changes_during_running_turn() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();

    let default_mode = collaboration_modes::default_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    chat.submit_user_message_with_mode("Implement the plan.".to_string(), default_mode);

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert!(chat.queued_user_messages.is_empty());
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
    let rendered = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Cannot switch collaboration mode while a turn is running."),
        "expected running-turn error message, got: {rendered:?}"
    );
}

#[tokio::test]
async fn submit_user_message_blocks_when_thread_model_is_unavailable() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("");
    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_no_submit_op(&mut op_rx);
    let rendered = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Thread model is unavailable."),
        "expected unavailable-model error, got: {rendered:?}"
    );
}

#[tokio::test]
async fn submit_user_message_with_mode_allows_same_mode_during_running_turn() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask.clone());
    chat.on_task_started();

    chat.submit_user_message_with_mode("Continue planning.".to_string(), plan_mask);

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert!(chat.queued_user_messages.is_empty());
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Plan,
                    ..
                }),
            personality: None,
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with plan collab mode, got {other:?}")
        }
    }
}

#[tokio::test]
async fn submit_user_message_with_mode_submits_when_plan_stream_is_not_active() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    let default_mode = collaboration_modes::default_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    let expected_mode = default_mode
        .mode
        .expect("expected default collaboration mode kind");
    chat.submit_user_message_with_mode("Implement the plan.".to_string(), default_mode);

    assert_eq!(chat.active_collaboration_mode_kind(), expected_mode);
    assert!(chat.queued_user_messages.is_empty());
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode: Some(CollaborationMode { mode, .. }),
            personality: None,
            ..
        } => assert_eq!(mode, expected_mode),
        other => {
            panic!("expected Op::UserTurn with default collab mode, got {other:?}")
        }
    }
}

#[tokio::test]
async fn plan_implementation_popup_skips_replayed_turn_complete() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.replay_initial_messages(vec![EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "turn-1".to_string(),
        last_agent_message: Some("Plan details".to_string()),
        completed_at: None,
        duration_ms: None,
    })]);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no plan popup for replayed turn, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_shows_once_when_replay_precedes_live_turn_complete() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    chat.on_plan_delta("- Step 1\n- Step 2\n".to_string());
    chat.on_plan_item_completed("- Step 1\n- Step 2\n".to_string());

    chat.replay_initial_messages(vec![EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "turn-1".to_string(),
        last_agent_message: Some("Plan details".to_string()),
        completed_at: None,
        duration_ms: None,
    })]);
    let replay_popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !replay_popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no prompt for replayed turn completion, got {replay_popup:?}"
    );

    chat.handle_codex_event(Event {
        id: "live-turn-complete-1".to_string(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Plan details".to_string()),
            completed_at: None,
            duration_ms: None,
        }),
    });

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected prompt for first live turn completion after replay, got {popup:?}"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let dismissed_popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !dismissed_popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected prompt to dismiss on Esc, got {dismissed_popup:?}"
    );

    chat.handle_codex_event(Event {
        id: "live-turn-complete-2".to_string(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Plan details".to_string()),
            completed_at: None,
            duration_ms: None,
        }),
    });
    let duplicate_popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !duplicate_popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no prompt for duplicate live completion, got {duplicate_popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_skips_when_messages_queued() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.queue_user_message("Queued message".into());

    chat.on_task_complete(Some("Plan details".to_string()), /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no plan popup with queued messages, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_skips_without_proposed_plan() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    chat.on_plan_update(UpdatePlanArgs {
        explanation: None,
        plan: vec![PlanItemArg {
            step: "First".to_string(),
            status: StepStatus::Pending,
        }],
    });
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no plan popup without proposed plan output, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_shows_after_proposed_plan_output() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    chat.on_plan_delta("- Step 1\n- Step 2\n".to_string());
    chat.on_plan_item_completed("- Step 1\n- Step 2\n".to_string());
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected plan popup after proposed plan output, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_skips_when_steer_follows_proposed_plan() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.thread_id = Some(ThreadId::new());

    chat.on_task_started();
    chat.on_plan_item_completed(
        "- Step 1
- Step 2
"
        .to_string(),
    );
    chat.bottom_pane
        .set_composer_text("Please continue.".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "Please continue.".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    complete_user_message(&mut chat, "user-1", "Please continue.");
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected no plan popup after a steer follows the plan, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_shows_after_new_plan_follows_steer() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.thread_id = Some(ThreadId::new());

    chat.on_task_started();
    chat.on_plan_item_completed(
        "- Initial plan
"
        .to_string(),
    );
    chat.bottom_pane
        .set_composer_text("Please revise.".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "Please revise.".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    complete_user_message(&mut chat, "user-1", "Please revise.");
    chat.on_plan_item_completed(
        "- Revised plan
"
        .to_string(),
    );
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected plan popup after a newer plan follows the steer, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_implementation_popup_skips_when_rate_limit_prompt_pending() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.has_chatgpt_account = true;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    chat.on_plan_update(UpdatePlanArgs {
        explanation: None,
        plan: vec![PlanItemArg {
            step: "First".to_string(),
            status: StepStatus::Pending,
        }],
    });
    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.on_task_complete(/*last_agent_message*/ None, /*from_replay*/ false);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Approaching rate limits"),
        "expected rate limit popup, got {popup:?}"
    );
    assert!(
        !popup.contains(PLAN_IMPLEMENTATION_TITLE),
        "expected plan popup to be skipped, got {popup:?}"
    );
}

#[tokio::test]
async fn plan_completion_restores_status_indicator_after_streaming_plan_output() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.on_task_started();
    assert_eq!(chat.bottom_pane.status_indicator_visible(), true);

    chat.on_plan_delta("- Step 1\n".to_string());
    chat.on_commit_tick();
    drain_insert_history(&mut rx);

    assert_eq!(chat.bottom_pane.status_indicator_visible(), false);
    assert_eq!(chat.bottom_pane.is_task_running(), true);

    chat.on_plan_item_completed("- Step 1\n".to_string());

    assert_eq!(chat.bottom_pane.status_indicator_visible(), true);
    assert_eq!(chat.bottom_pane.is_task_running(), true);
}

#[tokio::test]
async fn submit_user_message_queues_while_compaction_turn_is_running() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
                started_at: Some(0),
                completed_at: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );

    chat.submit_user_message(UserMessage::from("queued while compacting"));

    assert_eq!(chat.pending_steers.len(), 1);
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while compacting".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected running-turn compact steer submit, got {other:?}"),
    }

    chat.handle_codex_event(Event {
        id: "steer-rejected".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a compact turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Compact,
            }),
        }),
    });

    assert!(chat.pending_steers.is_empty());
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["queued while compacting"]
    );

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while compacting".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued compact follow-up Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn submit_user_message_emits_structured_plugin_mentions_from_bindings() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        cwd: PathBuf::from("/home/user/project"),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);
    chat.bottom_pane.set_plugin_mentions(Some(vec![
        crate::legacy_core::plugins::PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "Sample Plugin".to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        },
    ]));

    chat.submit_user_message(UserMessage {
        text: "$sample".to_string(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        text_elements: Vec::new(),
        mention_bindings: vec![MentionBinding {
            mention: "sample".to_string(),
            path: "plugin://sample@test".to_string(),
        }],
    });

    let Op::UserTurn { items, .. } = next_submit_op(&mut op_rx) else {
        panic!("expected Op::UserTurn");
    };
    assert_eq!(
        items,
        vec![
            UserInput::Text {
                text: "$sample".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Mention {
                name: "Sample Plugin".to_string(),
                path: "plugin://sample@test".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn enter_submits_when_plan_stream_is_not_active() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("submitted immediately".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test]
async fn collab_mode_shift_tab_cycles_only_when_idle() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let initial = chat.current_collaboration_mode().clone();
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.current_collaboration_mode(), &initial);

    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_collaboration_mode(), &initial);

    chat.on_task_started();
    let before = chat.active_collaboration_mode_kind();
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert_eq!(chat.active_collaboration_mode_kind(), before);
}

#[tokio::test]
async fn mode_switch_surfaces_model_change_notification_when_effective_model_changes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let default_model = chat.current_model().to_string();

    let mut plan_mask =
        collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
            .expect("expected plan collaboration mode");
    plan_mask.model = Some("gpt-5.1-codex-mini".to_string());
    chat.set_collaboration_mask(plan_mask);

    let plan_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        plan_messages.contains("Model changed to gpt-5.1-codex-mini medium for Plan mode."),
        "expected Plan-mode model switch notice, got: {plan_messages:?}"
    );

    let default_mask = collaboration_modes::default_mask(chat.model_catalog.as_ref())
        .expect("expected default collaboration mode");
    chat.set_collaboration_mask(default_mask);

    let default_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    let expected_default_message =
        format!("Model changed to {default_model} default for Default mode.");
    assert!(
        default_messages.contains(&expected_default_message),
        "expected Default-mode model switch notice, got: {default_messages:?}"
    );
}

#[tokio::test]
async fn mode_switch_surfaces_reasoning_change_notification_when_model_stays_same() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));

    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    chat.set_collaboration_mask(plan_mask);

    let plan_messages = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        plan_messages.contains("Model changed to gpt-5.3-codex medium for Plan mode."),
        "expected reasoning-change notice in Plan mode, got: {plan_messages:?}"
    );
}

#[tokio::test]
async fn collab_slash_command_opens_picker_and_updates_mode() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Collab);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Select Collaboration Mode"),
        "expected collaboration picker: {popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let selected_mask = match rx.try_recv() {
        Ok(AppEvent::UpdateCollaborationMode(mask)) => mask,
        other => panic!("expected UpdateCollaborationMode event, got {other:?}"),
    };
    chat.set_collaboration_mask(selected_mask);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with code collab mode, got {other:?}")
        }
    }

    chat.bottom_pane
        .set_composer_text("follow up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with code collab mode, got {other:?}")
        }
    }
}

#[tokio::test]
async fn plan_slash_command_switches_to_plan_mode() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let initial = chat.current_collaboration_mode().clone();

    chat.dispatch_command(SlashCommand::Plan);

    while let Ok(event) = rx.try_recv() {
        assert!(
            matches!(event, AppEvent::InsertHistoryCell(_)),
            "plan should not emit a non-history app event: {event:?}"
        );
    }
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.current_collaboration_mode(), &initial);
}

#[tokio::test]
async fn plan_slash_command_with_args_submits_prompt_in_plan_mode() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: ThreadId::new(),
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        cwd: PathBuf::from("/home/user/project"),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: None,
    };
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    chat.bottom_pane
        .set_composer_text("/plan build the plan".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        UserInput::Text {
            text: "build the plan".to_string(),
            text_elements: Vec::new(),
        }
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn collaboration_modes_defaults_to_code_on_startup() {
    let codex_home = tempdir().expect("tempdir");
    let cfg = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![(
            "features.collaboration_modes".to_string(),
            TomlValue::Boolean(true),
        )])
        .build()
        .await
        .expect("config");
    let resolved_model = crate::legacy_core::test_support::get_model_offline(cfg.model.as_deref());
    let session_telemetry = test_session_telemetry(&cfg, resolved_model.as_str());
    let init = ChatWidgetInit {
        config: cfg.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(unbounded_channel::<AppEvent>().0),
        initial_user_message: None,
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        model_catalog: test_model_catalog(&cfg),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: true,
        status_account_display: None,
        initial_plan_type: None,
        model: Some(resolved_model.clone()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        session_telemetry,
    };

    let chat = ChatWidget::new_with_app_event(init);
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_model(), resolved_model);
}

#[tokio::test]
async fn set_model_updates_active_collaboration_mask() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_model("gpt-5.1-codex-mini");

    assert_eq!(chat.current_model(), "gpt-5.1-codex-mini");
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn set_reasoning_effort_updates_active_collaboration_mask() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_reasoning_effort(/*effort*/ None);

    assert_eq!(
        chat.current_reasoning_effort(),
        Some(ReasoningEffortConfig::Medium)
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn set_reasoning_effort_does_not_override_active_plan_override() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1")).await;
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Low));

    assert_eq!(
        chat.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
}

#[tokio::test]
async fn collab_mode_is_sent_after_enabling() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn, got {other:?}")
        }
    }
}

#[tokio::test]
async fn collab_mode_applies_default_preset() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode:
                Some(CollaborationMode {
                    mode: ModeKind::Default,
                    ..
                }),
            personality: Some(Personality::Pragmatic),
            ..
        } => {}
        other => {
            panic!("expected Op::UserTurn with default collaboration_mode, got {other:?}")
        }
    }

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_collaboration_mode().mode, ModeKind::Default);
}

#[tokio::test]
async fn user_turn_includes_personality_from_config() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.set_feature_enabled(Feature::Personality, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("gpt-5.2-codex");
    chat.set_personality(Personality::Friendly);

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            personality: Some(Personality::Friendly),
            ..
        } => {}
        other => panic!("expected Op::UserTurn with friendly personality, got {other:?}"),
    }
}

#[tokio::test]
async fn plan_update_renders_history_cell() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let update = UpdatePlanArgs {
        explanation: Some("Adapting plan".to_string()),
        plan: vec![
            PlanItemArg {
                step: "Explore codebase".into(),
                status: StepStatus::Completed,
            },
            PlanItemArg {
                step: "Implement feature".into(),
                status: StepStatus::InProgress,
            },
            PlanItemArg {
                step: "Write tests".into(),
                status: StepStatus::Pending,
            },
        ],
    };
    chat.handle_codex_event(Event {
        id: "sub-1".into(),
        msg: EventMsg::PlanUpdate(update),
    });
    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected plan update cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Updated Plan"),
        "missing plan header: {blob:?}"
    );
    assert!(blob.contains("Explore codebase"));
    assert!(blob.contains("Implement feature"));
    assert!(blob.contains("Write tests"));
}
