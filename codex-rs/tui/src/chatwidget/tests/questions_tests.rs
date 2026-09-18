//! Verify question input delivery, retained drafts, and visible editor behavior.
use super::*;
use crate::bottom_pane::RestrictedInputMode;
use codex_protocol::items::AsyncUserInputQuestion;
use pretty_assertions::assert_eq;

fn questions() -> Vec<AsyncUserInputQuestion> {
    vec![
        question("Which way?", /*options*/ None),
        question("Any details?", /*options*/ None),
    ]
}

#[tokio::test]
async fn unavailable_send_keeps_answer_and_skip_remains_available() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.add_async_questions("message", &questions());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("kept answer".into());
    chat.blocks_direct_input = true;
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_eq!(question_count(&chat), 2);
    chat.handle_key_event(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
    assert_eq!(question_count(&chat), 1);
}

#[tokio::test]
async fn accepted_question_answer_uses_existing_delivery_and_keeps_main_draft() {
    for queued in [false, true] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.bottom_pane
            .set_composer_text("main draft".into(), Vec::new(), Vec::new());
        chat.input_queue.suppress_queue_autosend = queued;
        chat.add_async_questions("message", &questions());
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(question_count(&chat), 2);
        if !queued {
            insta::assert_snapshot!(
                "freeform_question",
                render_bottom_popup(&chat, /*width*/ 80)
            );
        }
        chat.bottom_pane.handle_paste("!literal answer".into());
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(question_count(&chat), 1);
        assert_eq!(chat.bottom_pane.composer_text(), "main draft");
        let expected = "> Which way?\n\n!literal answer";
        if queued {
            insta::assert_snapshot!(
                "queued_async_question_reply",
                render_bottom_popup(&chat, /*width*/ 80)
            );
            let restored = chat.pop_latest_queued_composer_state().unwrap();
            assert_eq!(restored.text, expected);
            assert!(op_rx.try_recv().is_err());
        } else {
            assert_answer(op_rx.try_recv().unwrap(), expected);
        }
    }
}

#[tokio::test]
async fn async_question_answers_preserve_ambiguous_skill_selection_and_dismiss_remotely() {
    use codex_context_fragments::AnsweredQuestion;
    use codex_context_fragments::ContextualUserFragment;

    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    let skill = SkillMetadata {
        name: "route".into(),
        description: "Choose a route".into(),
        short_description: None,
        interface: None,
        dependencies: None,
        path: test_path_buf("/tmp/route/SKILL.md").abs(),
        scope: crate::test_support::skill_scope_repo(),
        enabled: true,
        plugin_id: None,
    };
    let mut duplicate = skill.clone();
    duplicate.path = test_path_buf("/tmp/other-route/SKILL.md").abs();
    chat.set_skills(Some(vec![skill.clone(), duplicate]));
    chat.add_async_questions("message", &questions());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("Use $route".into());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let Op::UserTurn { items, .. } = ops.try_recv().unwrap() else {
        panic!("user turn")
    };
    let id = serde_json::json!(["request_user_input_async", "message", 0]).to_string();
    assert_eq!(
        items,
        vec![
            UserInput::Text {
                text: AnsweredQuestion::new(&id, "Which way?", "Use $route").render(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: skill.name,
                path: skill.path.to_path_buf()
            },
        ]
    );
    let (mut other, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    other.add_async_questions("message", &questions());
    complete_user_message_for_inputs(&mut other, "answer", items);
    assert_eq!(question_count(&other), 1);
}

#[tokio::test]
async fn ordinary_follow_up_clears_unanswered_questions_after_accepted_input() {
    for queued in [false, true] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.show_welcome_banner = false;
        chat.add_async_questions("old", &questions());
        if queued {
            chat.on_task_started();
        }
        chat.bottom_pane
            .set_composer_text("New prompt".into(), Vec::new(), Vec::new());
        chat.handle_key_event(KeyEvent::from(if queued {
            KeyCode::Tab
        } else {
            KeyCode::Enter
        }));

        assert_eq!(question_count(&chat), 0);
        if queued {
            assert_eq!(
                chat.input_queue.queued_user_messages.front().unwrap().text,
                "New prompt"
            );
            assert!(op_rx.try_recv().is_err());
        } else {
            assert_answer(op_rx.try_recv().unwrap(), "New prompt");
            insta::assert_snapshot!(
                "questions_cleared_by_follow_up",
                render_bottom_popup(&chat, /*width*/ 80)
                    .lines()
                    .next()
                    .unwrap()
            );
        }
        chat.add_async_questions("old", &questions());
        assert_eq!(question_count(&chat), 0);
        chat.add_async_questions("new", &[question("New question?", /*options*/ None)]);
        assert_eq!(question_count(&chat), 1);
    }
}

#[tokio::test]
async fn local_shell_command_preserves_unanswered_questions() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.bottom_pane
        .set_composer_text("!echo hi".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_eq!(question_count(&chat), 2);
    assert_matches!(
        op_rx.try_recv(),
        Ok(Op::RunUserShellCommand { command }) if command == "echo hi"
    );
}

#[tokio::test]
async fn queued_prompt_clears_questions_arriving_after_enqueue_when_it_starts() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.add_async_questions("old", &questions());
    chat.bottom_pane
        .set_composer_text("New prompt".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Tab));
    assert_eq!(question_count(&chat), 0);

    chat.add_async_questions("late", &questions());
    assert_eq!(question_count(&chat), 2);
    chat.turn_lifecycle.finish();
    chat.update_task_running_state();
    assert!(chat.maybe_send_next_queued_input());
    assert_answer(op_rx.try_recv().unwrap(), "New prompt");
    assert_eq!(question_count(&chat), 0);
    chat.add_async_questions("late", &questions());
    assert_eq!(question_count(&chat), 0);
}

#[tokio::test]
async fn queued_question_answer_preserves_other_questions_on_delivery() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.input_queue.suppress_queue_autosend = true;
    chat.add_async_questions("old", &questions());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("answer".into());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_eq!(question_count(&chat), 1);

    chat.input_queue.suppress_queue_autosend = false;
    assert!(chat.maybe_send_next_queued_input());
    assert_answer(op_rx.try_recv().unwrap(), "> Which way?\n\nanswer");
    assert_eq!(question_count(&chat), 1);
}

#[tokio::test]
async fn recovered_question_answers_preserve_other_questions() {
    for restore_thread in [false, true] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.on_task_started();
        chat.add_async_questions("old", &questions());
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        chat.bottom_pane.handle_paste("answer".into());
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_answer(op_rx.try_recv().unwrap(), "> Which way?\n\nanswer");
        assert_eq!(question_count(&chat), 1);
        chat.add_async_questions("late", &[question("Late?", /*options*/ None)]);

        if restore_thread {
            let state = chat.capture_thread_input_state().unwrap();
            chat.restore_thread_input_state(
                Some(state),
                ThreadInputStateRestoreMode {
                    preserve_in_flight_turn: false,
                },
            );
        } else {
            assert!(chat.enqueue_rejected_steer());
            chat.turn_lifecycle.finish();
            chat.update_task_running_state();
        }
        assert!(chat.maybe_send_next_queued_input());
        assert_answer(op_rx.try_recv().unwrap(), "> Which way?\n\nanswer");
        assert_eq!(question_count(&chat), 2);
    }
}

#[tokio::test]
async fn slash_and_mode_prompts_clear_unanswered_questions_after_delivery() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.handle_composer_input_result(
        InputResult::CommandWithArgs(SlashCommand::Plan, "New plan".into(), Vec::new()),
        /*had_modal_or_popup*/ false,
    );
    assert_answer(op_rx.try_recv().unwrap(), "New plan");
    assert_eq!(question_count(&chat), 0);

    chat.input_queue.user_turn_pending_start = false;
    chat.add_async_questions("next", &questions());
    let mode = collaboration_modes::default_mask(chat.model_catalog.as_ref()).unwrap();
    chat.submit_user_message_with_mode("Implement".to_string(), mode);
    assert_answer(op_rx.try_recv().unwrap(), "Implement");
    assert_eq!(question_count(&chat), 0);
}

#[tokio::test]
async fn accepted_review_clears_questions_but_rejected_review_preserves_them() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.handle_composer_input_result(
        InputResult::CommandWithArgs(SlashCommand::Review, "check this".into(), Vec::new()),
        /*had_modal_or_popup*/ false,
    );
    assert_matches!(
        op_rx.try_recv(),
        Ok(Op::Review {
            target: ReviewTarget::Custom { instructions }
        }) if instructions == "check this"
    );
    assert_eq!(question_count(&chat), 0);

    chat.add_async_questions("new", &questions());
    chat.blocks_direct_input = true;
    chat.handle_composer_input_result(
        InputResult::CommandWithArgs(SlashCommand::Review, "try again".into(), Vec::new()),
        /*had_modal_or_popup*/ false,
    );
    assert_eq!(question_count(&chat), 2);
    assert!(op_rx.try_recv().is_err());
}

#[tokio::test]
async fn queued_model_slash_prompt_clears_questions_but_local_slash_command_does_not() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.add_async_questions("old", &questions());
    for text in ["/model", "/goal clear"] {
        chat.handle_composer_input_result(
            InputResult::Queued {
                text: text.into(),
                text_elements: Vec::new(),
                action: QueuedInputAction::ParseSlash,
                pending_pastes: Vec::new(),
            },
            /*had_modal_or_popup*/ false,
        );
        assert_eq!(question_count(&chat), 2);
    }
    chat.handle_composer_input_result(
        InputResult::Queued {
            text: "/plan New work".into(),
            text_elements: Vec::new(),
            action: QueuedInputAction::ParseSlash,
            pending_pastes: Vec::new(),
        },
        /*had_modal_or_popup*/ false,
    );
    assert_eq!(question_count(&chat), 0);
    assert!(op_rx.try_recv().is_err());

    chat.add_async_questions("new", &questions());
    chat.handle_composer_input_result(
        InputResult::Queued {
            text: "/diff explain the change".into(),
            text_elements: Vec::new(),
            action: QueuedInputAction::ParseSlash,
            pending_pastes: Vec::new(),
        },
        /*had_modal_or_popup*/ false,
    );
    assert_eq!(question_count(&chat), 0);
}

#[tokio::test]
async fn rejected_synchronous_literal_prompt_keeps_questions() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.set_model("");
    chat.handle_composer_input_result(
        InputResult::Queued {
            text: "literal prompt".into(),
            text_elements: Vec::new(),
            action: QueuedInputAction::Literal,
            pending_pastes: Vec::new(),
        },
        /*had_modal_or_popup*/ false,
    );
    assert_eq!(question_count(&chat), 2);
    assert!(op_rx.try_recv().is_err());
}

#[tokio::test]
async fn reconnect_preserves_buffered_question_answer_source() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("answer".into());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_answer(op_rx.try_recv().unwrap(), "> Which way?\n\nanswer");
    assert_eq!(question_count(&chat), 1);
    chat.pause_for_disconnect();
    let state = chat.capture_thread_input_state();

    let (mut restored, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    restored.thread_id = Some(ThreadId::new());
    restored.restore_reconnected_input(state);
    assert_eq!(
        restored
            .input_queue
            .queued_user_messages
            .front()
            .unwrap()
            .source,
        UserMessageSource::QuestionAnswer,
    );
    // Recovered input stays paused until the user elects to retry it.
    restored.input_queue.recovered_queue = false;
    assert!(restored.maybe_send_next_queued_input());
    assert_answer(op_rx.try_recv().unwrap(), "> Which way?\n\nanswer");
    assert_eq!(question_count(&restored), 1);
}

#[tokio::test]
async fn blocked_follow_up_keeps_unanswered_questions() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.add_async_questions("old", &questions());
    chat.set_model("");
    chat.bottom_pane
        .set_composer_text("Blocked prompt".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_eq!(question_count(&chat), 2);
    assert!(op_rx.try_recv().is_err());
}

#[tokio::test]
async fn disconnected_questions_remain_editable_without_sending() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.add_async_questions("message", &questions());
    chat.pause_for_disconnect();
    chat.handle_restricted_key(
        KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
        RestrictedInputMode::Disconnected,
    );
    chat.bottom_pane.handle_paste("offline draft".into());
    chat.handle_restricted_key(
        KeyEvent::from(KeyCode::Enter),
        RestrictedInputMode::Disconnected,
    );
    assert_eq!(question_count(&chat), 2);
    assert!(op_rx.try_recv().is_err());
    chat.handle_restricted_key(
        KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL),
        RestrictedInputMode::Disconnected,
    );
    assert_eq!(question_count(&chat), 2);
}

fn question_count(chat: &ChatWidget) -> usize {
    chat.bottom_pane
        .questions
        .as_ref()
        .unwrap()
        .unanswered_count()
}

#[tokio::test]
async fn selected_answers_preserve_long_labels_and_reject_oversized_submissions() {
    for (length, snapshot) in [
        (300, "named_question"),
        (
            codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS,
            "oversized_question",
        ),
    ] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        let label = format!("{} but do not deploy", "x".repeat(length));
        chat.add_async_questions(
            "message",
            &[question("What next?", Some(vec![label.clone()]))],
        );
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        insta::assert_snapshot!(snapshot, render_bottom_popup(&chat, /*width*/ 80));
        let saved = question_count(&chat);
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        if length > 512 {
            assert_eq!(question_count(&chat), saved);
            assert!(ops.try_recv().is_err());
            // JSON escaping must count toward the limit even when the answer itself fits.
            chat.bottom_pane.handle_paste(
                "\"".repeat(codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS / 2),
            );
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert_eq!(question_count(&chat), saved);
            let rendered = render_bottom_popup(&chat, /*width*/ 80);
            insta::assert_snapshot!(rendered.lines().find(|line| line.contains("Answer too long")).unwrap(), @"  Answer too long; shorten it before sending");
        } else {
            assert_answer(ops.try_recv().unwrap(), &format!("> What next?\n\n{label}"));
        }
    }
}

#[tokio::test]
async fn question_inputs_support_vim_editing_and_search() {
    for (name, options) in [("single", None), ("other", Some(vec!["Named".to_string()]))] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.show_welcome_banner = false;
        chat.bottom_pane.set_vim_enabled(/*enabled*/ true);
        chat.add_async_questions("message", &[question("Which way?", options.clone())]);
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        if options.is_some() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
        }
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('i')));
        chat.bottom_pane
            .handle_paste("alpha beta\nalpha gamma".into());
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        for ch in "0k".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        assert!(chat.bottom_pane.questions.as_ref().unwrap().expanded);
        let normal = render_bottom_popup(&chat, /*width*/ 80);
        let editor = chat.bottom_pane.questions.as_mut().unwrap();
        editor.delivery_enabled = false;
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(render_bottom_popup(&chat, /*width*/ 80), normal);
        let editor = chat.bottom_pane.questions.as_mut().unwrap();
        editor.delivery_enabled = true;
        for ch in "dw".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("alpha beta"));
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('u')));
        assert!(render_bottom_popup(&chat, /*width*/ 80).contains("alpha beta"));
        chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("alpha beta"));
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('u')));
        for ch in "/gamma".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        let search = render_bottom_popup(&chat, /*width*/ 80);
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(question_count(&chat), 1);
        assert!(op_rx.try_recv().is_err());
        for ch in "ciw".chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        chat.bottom_pane.handle_paste("delta".into());
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_answer(
            op_rx.try_recv().unwrap(),
            "> Which way?\n\nalpha beta\nalpha delta",
        );
        insta::assert_snapshot!(
            format!("question_vim_{name}"),
            format!("NORMAL\n{normal}\n\nSEARCH\n{search}")
        );
    }
}

#[tokio::test]
async fn question_history_search_uses_updated_bindings() {
    for (binding, word) in [("f12", "earlier"), ("ctrl-x r", "persistent")] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.bottom_pane
            .set_history_metadata(thread_id, /*log_id*/ 7, /*entry_count*/ 1);
        chat.bottom_pane.record_replayed_user_message_history(
            crate::bottom_pane::HistoryEntry::with_pending_and_remote(
                "[Image #2]earlier answer".into(),
                vec![TextElement::new((0..10).into(), Some("[Image #2]".into()))],
                vec![PathBuf::from("/tmp/image.png")],
                Vec::new(),
                vec!["https://example.com/remote.png".into()],
            ),
        );
        chat.add_async_questions("message", &questions());
        let config = toml::from_str(&format!(
            "[composer]\nhistory_search_previous = '{binding}'"
        ))
        .unwrap();
        let keymap = RuntimeKeymap::from_config(&config).unwrap();
        chat.apply_keymap_update(config, &keymap);
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        chat.bottom_pane.record_replayed_user_message_history(
            crate::bottom_pane::HistoryEntry::new("newest answer".into()),
        );
        for (key, expected) in [
            (KeyCode::Up, "newest answer"),
            (KeyCode::Up, "earlier answer"),
            (KeyCode::Down, "newest answer"),
        ] {
            chat.handle_key_event(KeyEvent::from(key));
            let rendered = render_bottom_popup(&chat, /*width*/ 80);
            assert!(
                rendered.contains(expected),
                "{key:?}: expected {expected}, got {rendered}"
            );
        }
        chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        // Chords reach the editor using the dispatch binding resolved by the app.
        let (key, modifiers) = keymap.composer.history_search_previous[0].parts();
        chat.handle_key_event(KeyEvent::new(key, modifiers));
        for ch in word.chars() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
        }
        if binding != "f12" {
            chat.bottom_pane.on_history_lookup_response(
                crate::app_event::HistoryLookupResponse::Entry {
                    offset: 0,
                    log_id: 7,
                    entry: Some("persistent answer".into()),
                },
            );
        }
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert!(op_rx.try_recv().is_err());
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_answer(
            op_rx.try_recv().unwrap(),
            &format!("> Which way?\n\n{word} answer"),
        );
    }
}

#[tokio::test]
async fn question_editor_keeps_working_status_and_queued_messages_visible() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.bottom_pane
        .set_composer_text("main draft".into(), Vec::new(), Vec::new());
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("queued follow-up".to_string()).into());
    chat.refresh_pending_input_preview();
    chat.add_async_questions("message", &questions());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    let open = render_bottom_popup(&chat, /*width*/ 80);
    let open = open.split("\n  1 of 2").next().unwrap().trim_end();
    chat.bottom_pane.set_task_running(/*running*/ false);
    assert!(chat.bottom_pane.questions.as_ref().unwrap().expanded);
    chat.input_queue.queued_user_messages.clear();
    chat.refresh_pending_input_preview();
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.bottom_pane.set_status_line_enabled(/*enabled*/ false);
    let completed = render_bottom_popup(&chat, /*width*/ 80);
    let completed = completed.split("\n\n›").next().unwrap();
    insta::assert_snapshot!(
        "questions_with_status_and_queue",
        format!("OPEN\n{open}\n\nONLY QUESTIONS\n{completed}")
    );
}

#[tokio::test]
async fn async_questions_reject_blank_answers_but_allow_explicit_skip() {
    for options in [None, Some(vec!["Named".to_string()])] {
        let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.add_async_questions("message", &[question("Which way?", options.clone())]);
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        if options.is_some() {
            chat.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
        }
        for text in ["", " \t\n"] {
            chat.bottom_pane.handle_paste(text.into());
            let before = render_bottom_popup(&chat, /*width*/ 80);
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert_eq!(render_bottom_popup(&chat, /*width*/ 80), before);
            assert!(chat.input_queue.queued_user_messages.is_empty());
            assert!(op_rx.try_recv().is_err());
        }
        chat.handle_key_event(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::CONTROL));
        assert_eq!(question_count(&chat), 0);
        assert!(op_rx.try_recv().is_err());
    }
}

fn open_questions(chat: &mut ChatWidget, options: Option<Vec<String>>) {
    chat.thread_id = Some(ThreadId::new());
    chat.on_agent_message_item_completed(
        AgentMessageItem {
            id: "review".into(),
            content: Vec::new(),
            phase: None,
            memory_citation: None,
            delivery: None,
            questions: Some(vec![
                AsyncUserInputQuestion {
                    title: "First?".into(),
                    options,
                },
                question("Second?", Some(vec!["Next".into()])),
            ]),
        },
        "turn",
        /*from_replay*/ false,
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    render_bottom_popup(chat, /*width*/ 80);
}

#[tokio::test]
async fn question_queue_key_does_not_steer_the_running_turn() {
    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    open_questions(&mut chat, /*options*/ None);
    chat.on_task_started();
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("kept".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("  later  ".into());
    chat.handle_key_event(KeyEvent::from(KeyCode::Tab));
    assert_eq!(
        crate::async_question_reply::display_text(
            &chat.input_queue.queued_user_messages.front().unwrap().text
        )
        .unwrap(),
        "> First?\n\nlater"
    );
    let mut repeat = KeyEvent::from(KeyCode::Tab);
    repeat.kind = KeyEventKind::Repeat;
    chat.handle_key_event(repeat);
    assert_eq!(question_count(&chat), 1);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn question_choices_deliver_digits_and_leading_letters() {
    for (key, text, expected) in [
        ('2', "", "Second"),
        ('j', "ust text", "just text"),
        ('k', "eep it", "keep it"),
    ] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        open_questions(&mut chat, Some(vec!["First".into(), "Second".into()]));
        chat.handle_key_event(KeyEvent::from(KeyCode::Char(key)));
        if !text.is_empty() {
            chat.bottom_pane.handle_paste(text.into());
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        }
        assert_answer(ops.try_recv().unwrap(), &format!("> First?\n\n{expected}"));
        assert_eq!(question_count(&chat), 1);
    }
}

#[tokio::test]
async fn question_key_repeats_do_not_consume_another_question() {
    for key in [KeyCode::Enter, KeyCode::Char('1')] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        open_questions(&mut chat, Some(vec!["First".into()]));
        chat.handle_key_event(KeyEvent::from(key));
        ops.try_recv().unwrap();
        let mut repeat = KeyEvent::from(key);
        repeat.kind = KeyEventKind::Repeat;
        chat.handle_key_event(repeat);
        assert!(ops.try_recv().is_err());
        assert_eq!(question_count(&chat), 1);
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert!(ops.try_recv().is_err());
    }
}

#[tokio::test]
async fn question_queue_pop_becomes_an_ordinary_composer_draft_and_clears_questions_on_send() {
    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    for text in ["older", "newer"] {
        chat.input_queue
            .queued_user_messages
            .push_back(UserMessage::from(text).into());
    }
    open_questions(&mut chat, /*options*/ None);
    let forward = KeyEvent::new(KeyCode::Up, KeyModifiers::ALT);
    chat.handle_key_event(forward);
    insta::assert_snapshot!(
        "last_question_with_queue",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(forward);
    assert_eq!(chat.bottom_pane.composer_text(), "newer");
    chat.bottom_pane.handle_paste(" edited".into());
    chat.handle_key_event(forward);
    assert!(chat.bottom_pane.questions.as_ref().unwrap().expanded);
    assert_eq!(chat.bottom_pane.composer_text(), "newer edited");
    chat.handle_key_event(forward);
    assert_eq!(chat.bottom_pane.composer_text(), "older");
    assert!(chat.input_queue.queued_user_messages.is_empty());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_answer(ops.try_recv().unwrap(), "older");
    chat.handle_key_event(forward);
    assert_eq!(question_count(&chat), 0);
    assert!(!chat.bottom_pane.questions.as_ref().unwrap().expanded);
    assert!(
        !chat
            .bottom_pane
            .questions
            .as_ref()
            .unwrap()
            .has_queued_messages
    );
}

#[tokio::test]
async fn question_drafts_survive_navigation_and_snapshot_replay() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane
        .set_composer_text("main draft".into(), Vec::new(), Vec::new());
    open_questions(&mut chat, /*options*/ None);
    chat.bottom_pane.handle_paste("first draft".into());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    chat.bottom_pane.handle_paste("second draft".into());
    let saved = chat.capture_thread_input_state();
    let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let item = AppServerThreadItem::AgentMessage {
        id: "buffered".into(),
        text: String::new(),
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: Some(vec![question("Buffered?", /*options*/ None)]),
    };
    for kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        chat.replay_thread_item(item.clone(), "turn".into(), kind);
        assert!(chat.bottom_pane.questions.is_none());
    }
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            item,
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            completed_at_ms: 0,
        }),
        Some(ReplayKind::ThreadSnapshot),
    );
    assert_eq!(question_count(&chat), 1);
    chat.restore_reconnected_input(saved);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("second draft"));
    chat.pause_unavailable_thread();
    chat.handle_question_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
    chat.handle_question_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(question_count(&chat), 3);
    assert!(ops.try_recv().is_err());
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("first draft"));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
    assert_eq!(chat.bottom_pane.composer_text(), "main draft");
}

#[tokio::test]
async fn single_question_spacing_with_working_status() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.add_async_questions("single", &[question("Only question?", /*options*/ None)]);
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("A typed answer".into());
    let rendered = render_bottom_popup(&chat, /*width*/ 80);
    let rows: Vec<_> = rendered.lines().collect();
    let question = rows
        .iter()
        .position(|line| line.trim() == "Only question?")
        .unwrap();
    assert!(rows[question - 2].contains("Queued follow-up inputs"));
    assert!(rows[question - 1].is_empty());
    assert_eq!(rows[question + 2].trim(), "A typed answer");
    assert!(rows[question + 3].is_empty());
    insta::assert_snapshot!("single_question_working_spacing", rendered);
}

fn assert_answer(op: Op, expected: &str) {
    let Op::UserTurn { items, .. } = op else {
        panic!("user turn")
    };
    let mut items = items;
    if let [UserInput::Text { text, .. }] = items.as_mut_slice() {
        *text = crate::async_question_reply::display_text(text).unwrap_or_else(|| text.clone());
    }
    assert_eq!(
        items,
        vec![UserInput::Text {
            text: expected.into(),
            text_elements: Vec::new()
        }]
    );
}

fn question(title: &str, options: Option<Vec<String>>) -> AsyncUserInputQuestion {
    AsyncUserInputQuestion {
        title: title.into(),
        options,
    }
}

#[tokio::test]
async fn questions_and_queued_messages_share_the_resolved_shortcut() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("queued".to_string()).into());
    chat.refresh_pending_input_preview();
    for binding in [key_hint::shift(KeyCode::Left), key_hint::alt(KeyCode::Up)] {
        chat.bottom_pane
            .set_queued_message_edit_binding(Some(binding.into()));
        let hint = binding.display_label();
        assert!(render_bottom_popup(&chat, /*width*/ 100).contains(&hint));
        chat.add_async_questions(&hint, &questions());
        assert!(render_bottom_popup(&chat, /*width*/ 100).contains(&format!("{hint} to answer")));
        let (key, modifiers) = binding.parts();
        chat.handle_key_event(KeyEvent::new(key, modifiers));
        insta::assert_snapshot!(
            format!("question_queue_hint_{}", key),
            render_bottom_popup(&chat, /*width*/ 100)
        );
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    }
}

#[tokio::test]
async fn desktop_async_answer_dismisses_only_its_question_and_preserves_the_other_draft() {
    for replay_kind in [None, Some(ReplayKind::ThreadSnapshot)] {
        let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        let questions = vec![
            question("Same title?", /*options*/ None),
            question("Same title?", /*options*/ None),
        ];
        chat.add_async_questions("questions", &questions);
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        chat.bottom_pane
            .questions
            .as_mut()
            .unwrap()
            .navigate(/*forward*/ true);
        chat.bottom_pane
            .handle_paste("My draft for the second question".into());
        let reply = r#"<send_user_message_question_reply>
[{"questionItemId":"[\"request_user_input_async\",\"questions\",0]","question":"Same title?","answer":"Answer from desktop"}]
</send_user_message_question_reply>"#;
        let reply = format!(
            "# Context from my IDE setup:\n\n## Open tabs:\n- lib.rs: src/lib.rs\n\n## My request for Codex:\n{reply}"
        );
        let item = AppServerThreadItem::UserMessage {
            id: "answer".into(),
            client_id: None,
            content: vec![UserInput::Text {
                text: reply,
                text_elements: Vec::new(),
            }],
        };
        chat.handle_server_notification(
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread".into(),
                turn_id: "turn".into(),
                completed_at_ms: 0,
                item: item.clone(),
            }),
            replay_kind,
        );
        assert_eq!(question_count(&chat), 1);
        let cells = crate::thread_transcript::thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &chat.config.cwd,
            [item],
            crate::thread_transcript::RawReasoningVisibility::Hidden,
            /*config*/ None,
        );
        insta::assert_snapshot!(
            "desktop_async_question_reply",
            lines_to_single_string(&cells[0].transcript_lines(/*width*/ 80))
        );
        insta::assert_snapshot!(
            "async_question_answered_on_another_client",
            render_bottom_popup(&chat, /*width*/ 80)
        );
        chat.thread_id = Some(ThreadId::new());
        chat.input_queue.suppress_queue_autosend = true;
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_eq!(question_count(&chat), 0);
        // Removing the first question must not renumber the surviving question's identity.
        let queued = &chat.input_queue.queued_user_messages.front().unwrap().text;
        let replies = crate::async_question_reply::parse(queued).unwrap();
        assert_eq!(
            replies[0].question_item_id,
            r#"["request_user_input_async","questions",1]"#
        );
        assert_eq!(
            crate::async_question_reply::display_text(queued).unwrap(),
            "> Same title?\n\nMy draft for the second question"
        );
    }
}

#[tokio::test]
async fn distinct_async_question_replies_with_identical_text_both_render() {
    use codex_context_fragments::AnsweredQuestion;
    use codex_context_fragments::ContextualUserFragment;

    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    drain_insert_history(&mut rx);
    for id in ["first", "first", "second"] {
        let items = [UserInput::Text {
            text: AnsweredQuestion::new(id, "Continue?", "Yes").render(),
            text_elements: Vec::new(),
        }];
        chat.on_committed_user_message(
            &items, /*client_id*/ None, /*from_replay*/ false, "turn",
        );
    }
    let history = drain_insert_history(&mut rx);
    assert_eq!(history.len(), 2);
}

#[tokio::test]
async fn retried_question_answers_keep_separate_envelopes_and_order() {
    for interrupt in [false, true] {
        let (mut chat, _rx, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.thread_id = Some(ThreadId::new());
        chat.on_task_started();
        chat.submit_user_message(UserMessage::from("before"));
        chat.add_async_questions("message", &questions());
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        for answer in ["North", "Bring a map"] {
            chat.bottom_pane.handle_paste(answer.into());
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        }
        chat.submit_user_message(UserMessage::from("after"));
        let mut original = Vec::new();
        while let Ok(op) = ops.try_recv() {
            let Op::UserTurn { items, .. } = op else {
                panic!("user turn")
            };
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("one text input")
            };
            original.push(text.clone());
        }
        assert_eq!(original.len(), 4);
        let mut retried = Vec::new();
        if interrupt {
            chat.input_queue.submit_pending_steers_after_interrupt = true;
            chat.on_interrupted_turn(TurnAbortReason::Interrupted);
            let Op::UserTurn { items, .. } = ops.try_recv().unwrap() else {
                panic!("user turn")
            };
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("one text input")
            };
            retried.push(text.clone());
        } else {
            while !chat.input_queue.pending_steers.is_empty() {
                assert!(chat.enqueue_rejected_steer());
            }
        }
        while let Some((message, _)) = chat.pop_next_queued_user_message() {
            retried.push(message.text.clone());
        }
        assert_eq!(retried, original);
    }
}
