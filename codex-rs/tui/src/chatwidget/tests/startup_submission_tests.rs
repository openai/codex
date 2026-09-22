use super::*;
use pretty_assertions::assert_eq;

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
    let (mut source, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    source.bottom_pane.insert_str("keep the draft");
    source.bottom_pane.set_composer_cursor(/*cursor*/ 4);
    let mut draft = Some(source.bottom_pane.composer_draft_snapshot());
    let mut confirmed = true;
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    let before = chat.bottom_pane.composer_draft_snapshot();
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.thread_id = Some(ThreadId::new());
    chat.restore_startup_input_when_ready(&mut draft, &mut confirmed);
    assert_eq!(chat.bottom_pane.composer_draft_snapshot(), before);
    assert_no_submit_op(&mut op_rx);
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
    for action in ["edit", "disconnect", "parent-owned"] {
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
