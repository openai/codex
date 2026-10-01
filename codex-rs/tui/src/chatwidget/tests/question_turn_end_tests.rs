//! Live terminal turns recover open and collapsed question drafts into the composer.

use super::*;
use crate::bottom_pane::QueuedInputAction;
use codex_protocol::items::AsyncUserInputQuestion;
use pretty_assertions::assert_eq;

fn questions(chat: &mut ChatWidget, message_id: &str, title: &str) {
    chat.add_async_questions(
        message_id,
        &[AsyncUserInputQuestion {
            title: title.into(),
            options: None,
        }],
    );
}

#[tokio::test]
async fn question_turn_end_escapes_command_drafts_before_submission() {
    for (draft, escaped, queued_action) in [
        ("!echo partial", "\\!echo partial", QueuedInputAction::Plain),
        (
            "  !echo partial",
            "  \\!echo partial",
            QueuedInputAction::Plain,
        ),
        ("/", "\\/", QueuedInputAction::Plain),
        ("/diff", "\\/diff", QueuedInputAction::Plain),
        ("\t/diff", "\t\\/diff", QueuedInputAction::Plain),
        (" /diff", " /diff", QueuedInputAction::Plain),
        ("/tmp/file", "/tmp/file", QueuedInputAction::ParseSlash),
        ("\\/diff", "\\/diff", QueuedInputAction::Plain),
    ] {
        for queued in [false, true] {
            let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
            chat.thread_id = Some(ThreadId::new());
            chat.show_welcome_banner = false;
            handle_turn_started(&mut chat, "turn");
            questions(&mut chat, "question", "Which way?");
            chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
            chat.bottom_pane.handle_paste("answer".into());
            chat.bottom_pane
                .set_composer_text(draft.into(), Vec::new(), Vec::new());

            handle_turn_completed(&mut chat, "turn", /*duration_ms*/ None);
            let expected = format!("{escaped}\n\n> Which way?\n\nanswer");
            let submitted_text = expected.trim().to_string();
            assert_eq!(chat.bottom_pane.composer_text(), expected);
            if draft == "!echo partial" && !queued {
                insta::assert_snapshot!(
                    "question_turn_end_escaped_shell_command",
                    normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
                );
            }

            if queued {
                chat.on_task_started();
                chat.handle_key_event(KeyEvent::from(KeyCode::Tab));
                let queued_message = chat.input_queue.queued_user_messages.front().unwrap();
                assert_eq!(queued_message.text, submitted_text);
                assert_eq!(queued_message.action, queued_action);
                assert!(ops.try_recv().is_err());
            } else {
                chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
                let Op::UserTurn { items, .. } = ops.try_recv().unwrap() else {
                    panic!("expected user turn");
                };
                assert_eq!(
                    items,
                    vec![UserInput::Text {
                        text: submitted_text,
                        text_elements: Vec::new(),
                    }]
                );
            }
        }
    }
}

#[tokio::test]
async fn question_turn_end_appends_open_drafts_in_order_once() {
    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.show_welcome_banner = false;
    handle_turn_started(&mut chat, "turn");
    chat.bottom_pane
        .record_replayed_user_message_history(crate::bottom_pane::HistoryEntry::new(
            "Earlier answer".into(),
        ));
    let element = TextElement::new((0..5).into(), Some("$tool".into()));
    chat.bottom_pane.set_composer_text(
        "$tool existing draft".into(),
        vec![element.clone()],
        Vec::new(),
    );
    chat.bottom_pane.set_composer_cursor(/*cursor*/ 0);
    questions(&mut chat, "first", "Which style?");
    questions(&mut chat, "second", "What budget?");
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("  Over-ear  ".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("Under 250".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('e')));

    let recovered =
        "$tool existing draft\n\n> Which style?\n\nOver-ear\n\n> What budget?\n\nUnder 250";
    handle_turn_completed(&mut chat, "ended-turn", /*duration_ms*/ None);
    assert_eq!(
        (
            chat.bottom_pane.composer_text(),
            chat.bottom_pane.composer_text_elements(),
            chat.bottom_pane.question_editor().unanswered_count()
        ),
        (recovered.into(), vec![element], 0)
    );
    assert!(ops.try_recv().is_err());
    insta::assert_snapshot!(
        "question_turn_end_recovered_composer",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
    );

    handle_turn_completed(&mut chat, "ended-turn", /*duration_ms*/ None);
    assert_recovered_draft(&mut chat, recovered);

    questions(&mut chat, "large", "Which way?");
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    let large_answer = "long answer ".repeat(/*n*/ 200);
    chat.bottom_pane.handle_paste(large_answer.clone());
    handle_turn_completed(&mut chat, "ended-turn", /*duration_ms*/ None);
    assert_eq!(
        chat.bottom_pane.composer_text_with_pending(),
        format!("{recovered}\n\n> Which way?\n\n{}", large_answer.trim())
    );
    assert_eq!(chat.bottom_pane.composer_pending_pastes().len(), 1);
    insta::assert_snapshot!(
        "question_turn_end_large_draft",
        normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
    );
}

#[tokio::test]
async fn question_turn_end_recovers_collapsed_drafts_on_completion_and_failure() {
    for status in [AppServerTurnStatus::Completed, AppServerTurnStatus::Failed] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.bottom_pane.set_disable_paste_burst(/*disabled*/ false);
        handle_turn_started(&mut chat, "turn");
        questions(&mut chat, "question", "Which way?");
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        // Leave typing buffered, then collapse before completion.
        for ch in "answer".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
        for ch in "main".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        chat.bottom_pane.record_replayed_user_message_history(
            crate::bottom_pane::HistoryEntry::new("earlier prompt".into()),
        );
        chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('e')));
        if status == AppServerTurnStatus::Failed {
            chat.input_queue
                .queued_user_messages
                .push_back(UserMessage::from("queued prompt").into());
            handle_error(&mut chat, "Turn failed", /*codex_error_info*/ None);
            insta::assert_debug_snapshot!(
                chat.pending_notification.as_ref().map(Notification::display),
                @"None"
            );
            assert!(matches!(ops.try_recv().unwrap(), Op::UserTurn { .. }));
            assert_eq!(
                chat.capture_thread_input_state()
                    .unwrap()
                    .composer
                    .unwrap()
                    .text,
                "main\n\n> Which way?\n\nanswer"
            );
        }
        chat.handle_server_notification(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
                turn: app_server_turn(
                    "turn",
                    status.clone(),
                    /*duration_ms*/ None,
                    (status == AppServerTurnStatus::Failed).then(|| AppServerTurnError {
                        misalignment: None,
                        message: "Turn failed".into(),
                        codex_error_info: None,
                        additional_details: None,
                    }),
                ),
            }),
            /*replay_kind*/ None,
        );
        assert_eq!(chat.bottom_pane.composer_text(), "earlier prompt");
        assert!(!chat.bottom_pane.no_modal_or_popup_active());
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        assert_recovered_draft(&mut chat, "main\n\n> Which way?\n\nanswer");
        assert!(ops.try_recv().is_err());
    }
}

#[tokio::test]
async fn question_turn_end_recovers_after_interruption_restores_queued_input() {
    enum MainInput {
        HistoryAccept,
        HistoryCancel,
        BufferedTyping,
    }
    for main_input in [
        MainInput::HistoryAccept,
        MainInput::HistoryCancel,
        MainInput::BufferedTyping,
    ] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        handle_turn_started(&mut chat, "turn");
        chat.bottom_pane.set_disable_paste_burst(/*disabled*/ false);
        chat.bottom_pane
            .set_composer_text("main draft".into(), Vec::new(), Vec::new());
        questions(&mut chat, "question", "Which way?");
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        chat.bottom_pane.handle_paste("answer".into());
        chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
        let main_draft = match main_input {
            MainInput::HistoryAccept | MainInput::HistoryCancel => {
                chat.bottom_pane.record_replayed_user_message_history(
                    crate::bottom_pane::HistoryEntry::new("earlier prompt".into()),
                );
                chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
                chat.handle_key_event(KeyEvent::from(KeyCode::Char('e')));
                assert_eq!(chat.bottom_pane.composer_text(), "earlier prompt");
                if matches!(main_input, MainInput::HistoryAccept) {
                    "earlier prompt"
                } else {
                    "main draft"
                }
            }
            MainInput::BufferedTyping => {
                chat.handle_key_event(KeyEvent::from(KeyCode::Char('+')));
                "main draft+"
            }
        };
        chat.input_queue
            .queued_user_messages
            .push_back(UserMessage::from("queued prompt").into());
        chat.input_queue
            .pending_steers
            .push_back(pending_steer("pending steer"));
        handle_turn_interrupted(&mut chat, "turn");
        if !matches!(main_input, MainInput::BufferedTyping) {
            assert_eq!(chat.bottom_pane.composer_text(), "earlier prompt");
            assert!(!chat.bottom_pane.no_modal_or_popup_active());
            assert_eq!(
                chat.capture_thread_input_state()
                    .unwrap()
                    .composer
                    .unwrap()
                    .text,
                "pending steer\nqueued prompt\nmain draft\n\n> Which way?\n\nanswer"
            );
            // A query miss and unsuccessful Enter must retain both the search and the answers.
            chat.handle_key_event(KeyEvent::from(KeyCode::Char('z')));
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert!(!chat.bottom_pane.no_modal_or_popup_active());
            assert_eq!(chat.bottom_pane.composer_text(), "main draft");
            chat.handle_key_event(KeyEvent::from(KeyCode::Backspace));
            if matches!(main_input, MainInput::HistoryAccept) {
                insta::assert_snapshot!(
                    "question_turn_end_keeps_history_search",
                    normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80))
                );
            }
            chat.handle_key_event(match main_input {
                MainInput::HistoryAccept => KeyEvent::from(KeyCode::Enter),
                _ => KeyEvent::from(KeyCode::Esc),
            });
        }
        if matches!(main_input, MainInput::HistoryAccept) {
            assert_recovered_draft(&mut chat, "earlier prompt");
        } else {
            assert_recovered_draft(
                &mut chat,
                &format!("pending steer\nqueued prompt\n{main_draft}\n\n> Which way?\n\nanswer"),
            );
        }
        assert!(!chat.has_queued_follow_up_messages());
        assert!(ops.try_recv().is_err());
    }
}

#[tokio::test]
async fn question_turn_end_keeps_multiline_question_and_answer_as_text() {
    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn");
    questions(
        &mut chat,
        "question",
        "First line\r\n\r\n\x1b[31mLast\r line?\x1b[0m\x07",
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane
        .handle_paste("  First answer line\n\nLast answer line  ".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));

    handle_turn_completed(&mut chat, "turn", /*duration_ms*/ None);
    let recovered = "> First line\n>\n> Last line?\n\nFirst answer line\n\nLast answer line";
    assert_eq!(
        (
            chat.bottom_pane.composer_text_with_pending(),
            chat.bottom_pane.question_editor().unanswered_count(),
        ),
        (recovered.to_string(), 0),
    );
    assert!(ops.try_recv().is_err());

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let Op::UserTurn { items, .. } = ops.try_recv().unwrap() else {
        panic!("user turn")
    };
    assert_eq!(
        items,
        vec![UserInput::Text {
            text: recovered.into(),
            text_elements: Vec::new(),
        }]
    );
}

fn assert_recovered_draft(chat: &mut ChatWidget, text: &str) {
    assert_eq!(
        (
            chat.bottom_pane.composer_text(),
            chat.bottom_pane.question_editor().unanswered_count()
        ),
        (text.into(), 0)
    );
}
