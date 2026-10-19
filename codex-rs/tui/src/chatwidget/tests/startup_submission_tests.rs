use super::*;
use crate::app_event::HistoryLookupResponse;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn initial_prompt_recall_precedes_cross_session_history() {
    for submit_follow_up in [false, true] {
        let (mut chat, mut events, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.bottom_pane
            .set_history_metadata(thread_id, /*log_id*/ 7, /*entry_count*/ 1);
        chat.initial_user_message =
            create_initial_user_message(Some("CURRENT_A".into()), Vec::new(), Vec::new());

        chat.submit_initial_user_message_if_pending();
        chat.submit_initial_user_message_if_pending();
        assert_matches!(next_submit_op(&mut ops), Op::UserTurn { items, .. }
            if items == vec![UserInput::Text { text: "CURRENT_A".into(), text_elements: Vec::new() }]);
        assert_no_submit_op(&mut ops);
        handle_turn_started(&mut chat, "initial");
        handle_turn_completed(&mut chat, "initial", /*duration_ms*/ None);

        let mut expected = vec!["CURRENT_A"];
        if submit_follow_up {
            chat.handle_paste("CURRENT_B".into());
            chat.handle_key_event(KeyCode::Enter.into());
            assert_matches!(next_submit_op(&mut ops), Op::UserTurn { .. });
            handle_turn_started(&mut chat, "follow-up");
            handle_turn_completed(&mut chat, "follow-up", /*duration_ms*/ None);
            expected.insert(0, "CURRENT_B");
        }
        let persisted = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::AppendMessageHistoryEntry {
                    thread_id: owner,
                    text,
                } => {
                    assert_eq!(owner, thread_id);
                    Some(text)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            persisted,
            expected.iter().rev().copied().collect::<Vec<_>>()
        );

        for text in expected {
            chat.handle_key_event(KeyCode::Up.into());
            assert_eq!(chat.bottom_pane.composer_text(), text);
            assert!(
                events.try_recv().is_err(),
                "local recall must not fetch shared history"
            );
        }
        if submit_follow_up {
            assert_chatwidget_snapshot!(
                "initial_prompt_recalled_composer",
                super::helpers::normalize_snapshot_paths(render_bottom_popup(
                    &chat, /*width*/ 80
                ))
            );
        }
        chat.handle_key_event(KeyCode::Up.into());
        let AppEvent::LookupMessageHistoryEntry {
            thread_id: owner,
            log_id,
            offset,
        } = events
            .try_recv()
            .expect("shared history lookup after current prompts")
        else {
            panic!("expected shared history lookup");
        };
        assert_eq!((owner, log_id, offset), (thread_id, 7, 0));
        chat.bottom_pane
            .on_history_lookup_response(HistoryLookupResponse::Entry {
                log_id,
                offset,
                entry: Some("FOREIGN_X".into()),
            });
        assert_eq!(chat.bottom_pane.composer_text(), "FOREIGN_X");
    }
}

#[tokio::test]
async fn initial_prompt_image_preparation_preserves_recall_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    image::RgbImage::new(/*width*/ 1, /*height*/ 1)
        .save(&path)
        .unwrap();
    let placeholder = "[Image #1]";
    for (input, elements, recalled_text, placeholder_start) in [
        (
            "describe",
            Vec::new(),
            "describe [Image #1]",
            "describe ".len(),
        ),
        (
            "[Image #1] describe",
            vec![TextElement::new(
                (0..placeholder.len()).into(),
                Some(placeholder.into()),
            )],
            "[Image #1] describe",
            0,
        ),
    ] {
        let (mut chat, mut events, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.bottom_pane
            .set_history_metadata(thread_id, /*log_id*/ 7, /*entry_count*/ 1);
        chat.snapshot_local_images = true;
        let text_elements = vec![TextElement::new(
            (placeholder_start..placeholder_start + placeholder.len()).into(),
            Some(placeholder.into()),
        )];
        chat.initial_user_message =
            create_initial_user_message(Some(input.into()), vec![path.clone()], elements);

        chat.submit_initial_user_message_if_pending();
        assert!(chat.pending_image_submission.is_some());
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let AppEvent::ImagesPrepared(id) = events.recv().await.unwrap() {
                    chat.on_images_prepared(id);
                    break;
                }
            }
        })
        .await
        .expect("initial image preparation");
        assert_matches!(next_submit_op(&mut ops), Op::UserTurn { items, .. }
        if items.iter().any(|item| matches!(item, UserInput::Text { text, .. } if text == input)));
        chat.submit_initial_user_message_if_pending();
        assert_no_submit_op(&mut ops);
        handle_turn_started(&mut chat, "initial");
        handle_turn_completed(&mut chat, "initial", /*duration_ms*/ None);
        while events.try_recv().is_ok() {}

        chat.handle_key_event(KeyCode::Up.into());
        let recalled = chat.bottom_pane.composer_draft_snapshot();
        assert_eq!(
            (
                recalled.text,
                recalled.text_elements,
                recalled.local_images,
                recalled.pending_pastes
            ),
            (
                recalled_text.into(),
                text_elements,
                vec![LocalImageAttachment {
                    placeholder: placeholder.into(),
                    path: path.clone()
                }],
                Vec::new()
            ),
        );
        chat.handle_key_event(KeyCode::Up.into());
        assert_matches!(
            events.try_recv(),
            Ok(AppEvent::LookupMessageHistoryEntry { offset: 0, .. })
        );

        chat.handle_key_event(KeyCode::Enter.into());
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let AppEvent::ImagesPrepared(id) = events.recv().await.unwrap() {
                    chat.on_images_prepared(id);
                    break;
                }
            }
        })
        .await
        .expect("recalled image preparation");
        assert_matches!(next_submit_op(&mut ops), Op::UserTurn { items, .. }
        if items.iter().any(|item| matches!(item, UserInput::Image { .. })));
    }
}

#[tokio::test]
async fn initial_prompt_queued_before_session_is_recalled_once_after_dispatch() {
    let (mut chat, mut events, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.initial_user_message = Some("CURRENT_A".into());
    chat.submit_initial_user_message_if_pending();
    assert_no_submit_op(&mut ops);
    assert_eq!(chat.queued_user_message_texts(), vec!["CURRENT_A"]);

    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.bottom_pane
        .set_history_metadata(thread_id, /*log_id*/ 7, /*entry_count*/ 1);
    assert!(chat.maybe_send_next_queued_input());
    assert_matches!(next_submit_op(&mut ops), Op::UserTurn { .. });
    handle_turn_started(&mut chat, "initial");
    handle_turn_completed(&mut chat, "initial", /*duration_ms*/ None);
    while events.try_recv().is_ok() {}

    chat.handle_key_event(KeyCode::Up.into());
    assert_eq!(chat.bottom_pane.composer_text(), "CURRENT_A");
    chat.handle_key_event(KeyCode::Up.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::LookupMessageHistoryEntry { offset: 0, .. })
    );
}

#[tokio::test]
async fn rejected_initial_prompt_is_not_recorded_in_recall_history() {
    let (mut chat, mut events, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.bottom_pane
        .set_history_metadata(thread_id, /*log_id*/ 7, /*entry_count*/ 1);
    // A disconnected submission channel rejects the initial prompt synchronously.
    ops.close();
    chat.initial_user_message = Some("CURRENT_A".into());
    chat.submit_initial_user_message_if_pending();
    assert_no_submit_op(&mut ops);
    while events.try_recv().is_ok() {}

    chat.handle_key_event(KeyCode::Up.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::LookupMessageHistoryEntry { offset: 0, .. })
    );
}

#[tokio::test]
async fn startup_submission_waits_for_session_and_sends_exactly_once() {
    let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    source.bottom_pane.insert_str("confirmed before startup");
    let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
    let mut confirmed = true;
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert!(draft.is_none());
    assert_eq!(chat.bottom_pane.composer_text(), "confirmed before startup");
    for _ in 0..3 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    }
    for key in [
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT),
    ] {
        chat.handle_key_event(key);
    }
    assert_no_submit_op(&mut op_rx);
    chat.thread_id = Some(ThreadId::new());
    let model = chat.effective_collaboration_mode().model().to_string();
    chat.set_model("");
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert_no_submit_op(&mut op_rx);
    assert_eq!(chat.bottom_pane.composer_text(), "confirmed before startup");
    chat.set_model(&model);
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { items, .. } if items == vec![UserInput::Text {
        text: "confirmed before startup".into(), text_elements: Vec::new(),
    }]);
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert_no_submit_op(&mut op_rx);
    assert_eq!(chat.bottom_pane.composer_text(), "");
}

#[tokio::test]
async fn startup_submission_escape_restores_editing_without_submitting() {
    for rejected_startup in [false, true] {
        let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        source.bottom_pane.insert_str("keep the draft");
        source.bottom_pane.set_composer_cursor(/*cursor*/ 4);
        let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
        let mut confirmed = true;
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        let before = chat.bottom_pane.composer_draft_snapshot();
        if rejected_startup {
            chat.pause_for_rejected_startup();
            chat.handle_restricted_key(
                KeyEvent::from(KeyCode::Esc),
                crate::bottom_pane::RestrictedInputMode::Disconnected,
            );
            assert_snapshot!(
                render_bottom_popup(&chat, /*width*/ 80).lines().last().unwrap(),
                @"  ⌃c quit"
            );
            chat.input_queue.suppress_queue_autosend = false;
        } else {
            chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        }
        assert!(chat.input_queue.startup_submission.is_none());
        chat.thread_id = Some(ThreadId::new());
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        assert_eq!(chat.bottom_pane.composer_draft_snapshot(), before);
        assert_no_submit_op(&mut op_rx);
    }
}

#[tokio::test]
async fn startup_submission_waits_for_protected_view_and_preserves_paste_provenance() {
    for payload in [
        "  !echo hi\n# visible tail".to_string(),
        format!("!echo {}", "x".repeat(/*n*/ 1000)),
    ] {
        let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        source.handle_paste(payload.clone());
        let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
        let mut confirmed = true;
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.open_approvals_popup();
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        assert!(draft.is_some());
        assert_no_submit_op(&mut op_rx);
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        chat.thread_id = None;
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        chat.pause_for_rejected_startup();
        for key in [KeyCode::Null, KeyCode::Left, KeyCode::Right, KeyCode::Enter] {
            chat.handle_restricted_key(
                KeyEvent::from(key),
                crate::bottom_pane::RestrictedInputMode::Disconnected,
            );
        }
        assert_no_submit_op(&mut op_rx);
        chat.input_queue.suppress_queue_autosend = false;
        chat.thread_id = Some(ThreadId::new());
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        if payload.starts_with(' ') {
            assert!(op_rx.try_recv().is_err());
            assert_eq!(chat.bottom_pane.composer_text(), payload);
            chat.handle_key_event(KeyCode::Enter.into());
            assert_matches!(op_rx.try_recv(), Ok(Op::RunUserShellCommand { command })
                if command == payload.trim_start().trim_start_matches('!'));
        } else {
            assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { items, .. } if items == vec![UserInput::Text {
                text: payload, text_elements: Vec::new(),
            }]);
        }
        assert_no_submit_op(&mut op_rx);
    }
}

#[tokio::test]
async fn startup_submission_edit_or_disconnect_cancels_without_losing_draft() {
    for action in ["edit", "disconnect", "parent-owned", "rejected-edit-undo"] {
        let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        source.bottom_pane.insert_str("confirmed draft");
        let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
        let mut confirmed = true;
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        if action == "parent-owned" {
            chat.set_parent_owned_thread();
        }
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        if action == "disconnect" {
            chat.pause_for_disconnect();
        } else if action == "edit" {
            chat.handle_paste(" edited".into());
        } else if action == "rejected-edit-undo" {
            chat.pause_for_rejected_startup();
            for key in [KeyCode::Char('x'), KeyCode::Backspace] {
                chat.handle_restricted_key(
                    KeyEvent::from(key),
                    crate::bottom_pane::RestrictedInputMode::Disconnected,
                );
            }
            chat.input_queue.suppress_queue_autosend = false;
        }
        chat.thread_id = Some(ThreadId::new());
        chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
        assert_eq!(
            chat.bottom_pane.composer_text(),
            if action != "edit" {
                "confirmed draft"
            } else {
                "confirmed draft edited"
            }
        );
        assert!(chat.input_queue.startup_submission.is_none());
        assert_no_submit_op(&mut op_rx);
    }
}

#[tokio::test]
async fn startup_submission_does_not_confirm_merged_destination_text() {
    let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    source.bottom_pane.insert_str("startup draft");
    let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
    let mut confirmed = true;
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.insert_str("existing draft");
    chat.thread_id = Some(ThreadId::new());
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "existing draft\nstartup draft"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn voice_shortcut_cancels_startup_submission_before_app_dispatch() -> color_eyre::Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.bottom_pane.insert_str("confirmed draft");
    let mut draft = Some(app.chat_widget.bottom_pane.composer_draft_snapshot());
    app.chat_widget
        .bottom_pane
        .set_composer_text(String::new(), Vec::new(), Vec::new());
    app.chat_widget
        .restore_startup_input_when_ready(&mut draft, &mut true);
    assert!(app.chat_widget.input_queue.startup_submission.is_some());
    app.handle_tui_event(
        &mut tui,
        &mut server,
        crate::tui::TuiEvent::Key(KeyCode::F(8).into()),
    )
    .await?;
    assert!(app.chat_widget.input_queue.startup_submission.is_none());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "confirmed draft"
    );
    server.shutdown().await?;
    Ok(())
}
