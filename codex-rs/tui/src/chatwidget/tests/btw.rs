use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

pub(super) async fn forked_thread_history_line_includes_name_and_id_snapshot() -> String {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_next_fork_banner_parent_label(Some("named parent".to_string()));

    let forked_from_id =
        ThreadId::from_string("e9f18a88-8081-4e51-9d4e-8af5cde2d8dd").expect("forked id");
    chat.emit_forked_thread_event(forked_from_id);

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    lines_to_single_string(&history_cell.display_lines(/*width*/ 80))
}

pub(super) async fn forked_thread_history_line_without_name_shows_id_once_snapshot() -> String {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let forked_from_id =
        ThreadId::from_string("019c2d47-4935-7423-a190-05691f566092").expect("forked id");
    chat.emit_forked_thread_event(forked_from_id);

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    lines_to_single_string(&history_cell.display_lines(/*width*/ 80))
}

pub(super) async fn suppressed_interrupted_turn_notice_skips_history_warning() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_interrupted_turn_notice_mode(InterruptedTurnNoticeMode::Suppress);
    chat.on_task_started();
    chat.on_agent_message_delta("partial output".to_string());

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    let inserted = drain_insert_history(&mut rx);
    assert!(
        inserted.iter().all(|cell| {
            let rendered = lines_to_single_string(cell);
            !rendered.contains("Conversation interrupted - tell the model what to do differently.")
                && !rendered.contains("Model interrupted to submit steer instructions.")
        }),
        "unexpected interrupted-turn notice in BTW thread: {inserted:?}"
    );
}

fn assert_btw_rename_rejected(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let event = rx.try_recv().expect("expected BTW rename error");
    match event {
        AppEvent::InsertHistoryCell(cell) => {
            let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
            assert!(
                rendered.contains("BTW threads are ephemeral and cannot be renamed."),
                "expected BTW rename error, got {rendered:?}"
            );
        }
        other => panic!("expected InsertHistoryCell error, got {other:?}"),
    }
    assert!(rx.try_recv().is_err(), "expected no follow-up events");
    assert!(op_rx.try_recv().is_err(), "expected no rename op");
}

pub(super) async fn slash_rename_is_rejected_for_btw_threads() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_thread_rename_block_message(
        "BTW threads are ephemeral and cannot be renamed.".to_string(),
    );

    chat.dispatch_command(SlashCommand::Rename);
    assert_btw_rename_rejected(&mut rx, &mut op_rx);
}

pub(super) async fn slash_rename_with_args_is_rejected_for_btw_threads() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_thread_rename_block_message(
        "BTW threads are ephemeral and cannot be renamed.".to_string(),
    );

    chat.dispatch_command_with_args(SlashCommand::Rename, "investigate".to_string(), Vec::new());
    assert_btw_rename_rejected(&mut rx, &mut op_rx);
}

pub(super) async fn submit_user_message_as_plain_user_turn_does_not_run_shell_commands() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.submit_user_message_as_plain_user_turn("!echo hello".into());

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => pretty_assertions::assert_eq!(
            items,
            vec![UserInput::Text {
                text: "!echo hello".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn for BTW shell-like input, got {other:?}"),
    }
}

pub(super) async fn slash_btw_requests_forked_side_question_while_task_running() -> String {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let parent_thread_id = ThreadId::new();
    chat.thread_id = Some(parent_thread_id);
    chat.on_task_started();
    chat.show_welcome_banner = false;
    chat.bottom_pane.set_composer_text(
        "/btw explore the codebase".to_string(),
        Vec::new(),
        Vec::new(),
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches::assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::StartBtw {
            parent_thread_id: emitted_parent_thread_id,
            user_message,
        }) if emitted_parent_thread_id == parent_thread_id
            && user_message
                == UserMessage {
                    text: "explore the codebase".to_string(),
                    local_images: Vec::new(),
                    remote_image_urls: Vec::new(),
                    text_elements: Vec::new(),
                    mention_bindings: Vec::new(),
                }
    );
    assert!(
        op_rx.try_recv().is_err(),
        "expected no op on the parent thread"
    );

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw BTW footer");
    terminal.backend().to_string()
}

pub(super) async fn btw_footer_override_snapshot() -> String {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_thread_footer_hint_override(Some(vec![(
        "BTW".to_string(),
        "from main thread · Esc to return".to_string(),
    )]));

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw BTW footer");
    terminal.backend().to_string()
}

pub(super) async fn clearing_thread_footer_override_preserves_general_footer_hint_snapshot()
-> String {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_footer_hint_override(Some(vec![(
        "Save and close external editor to continue.".to_string(),
        String::new(),
    )]));
    chat.set_thread_footer_hint_override(Some(vec![(
        "BTW".to_string(),
        "from main thread · Esc to return".to_string(),
    )]));
    chat.set_thread_footer_hint_override(/*items*/ None);

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw preserved footer hint");
    terminal.backend().to_string()
}
