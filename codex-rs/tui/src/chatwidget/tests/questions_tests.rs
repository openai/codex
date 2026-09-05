//! Verify question input delivery, retained drafts, and visible editor behavior.
use super::*;
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
            assert_eq!(
                chat.input_queue.queued_user_messages.front().unwrap().text,
                expected
            );
            assert!(op_rx.try_recv().is_err());
        } else {
            assert_answer(op_rx.try_recv().unwrap(), expected);
        }
    }
}

#[tokio::test]
async fn disconnected_questions_remain_editable_without_sending() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.add_async_questions("message", &questions());
    chat.pause_for_disconnect();
    chat.handle_disconnected_key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
    chat.bottom_pane.handle_paste("offline draft".into());
    chat.handle_disconnected_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(question_count(&chat), 2);
    assert!(op_rx.try_recv().is_err());
    chat.handle_disconnected_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
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
            chat.bottom_pane
                .handle_paste("x".repeat(codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS));
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert_eq!(question_count(&chat), saved);
            let rendered = render_bottom_popup(&chat, /*width*/ 80);
            insta::assert_snapshot!(rendered.lines().find(|line| line.contains("Answer too long")).unwrap(), @"  Answer too long; limit 1048562 characters");
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
        chat.input_queue.queued_user_messages.front().unwrap().text,
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
    assert!(rows[question - 2].contains("Working"));
    assert!(rows[question - 1].is_empty());
    assert_eq!(rows[question + 2].trim(), "A typed answer");
    assert!(rows[question + 3].is_empty());
    insta::assert_snapshot!("single_question_working_spacing", rendered);
}

fn assert_answer(op: Op, expected: &str) {
    let Op::UserTurn { items, .. } = op else {
        panic!("user turn")
    };
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
