use super::*;
use crate::app::test_support::make_test_app_with_channels;
use pretty_assertions::assert_eq;

#[test]
fn side_boundary_prompt_marks_inherited_history_reference_only() {
    let item = App::side_boundary_prompt_item();
    let ResponseItem::Message { role, content, .. } = item else {
        panic!("expected hidden side boundary prompt to be a user message");
    };
    assert_eq!(role, "user");
    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected hidden side boundary prompt text");
    };
    assert!(text.contains("Side conversation boundary."));
    assert!(text.contains("Everything before this boundary is inherited history"));
    assert!(text.contains("It is not your current task."));
    assert!(text.contains("Only messages submitted after this boundary are active"));
    assert!(text.contains("Do not continue, execute, or complete"));
    assert!(text.contains("separate from the main thread"));
    assert!(text.contains("External tools may be available according to this thread's current"));
    assert!(text.contains("Any tool calls or outputs visible before this boundary happened"));
    assert!(text.contains("Sub-agents are off-limits in this side conversation."));
    assert!(text.contains("Do not modify files"));
}

#[test]
fn side_start_error_message_explains_missing_first_prompt() {
    let err = color_eyre::eyre::eyre!(
        "thread/fork failed during TUI bootstrap: thread/fork failed: no rollout found for thread id 019da1a1-bed9-7a43-88a2-b49d43915021"
    );

    assert_eq!(
        App::side_start_error_message(&err),
        "'/side' is unavailable until the current conversation has started. Send a message first, then try /side again."
    );
}

#[test]
fn side_start_error_message_uses_generic_start_wording() {
    let err = color_eyre::eyre::eyre!("transport disconnected");

    assert_eq!(
        App::side_start_error_message(&err),
        "Failed to start side conversation: transport disconnected"
    );
}

#[test]
fn side_developer_instructions_appends_existing_policy() {
    let developer_instructions =
        App::side_developer_instructions(Some("Existing developer policy."));

    assert!(developer_instructions.contains("Existing developer policy."));
    assert!(
        developer_instructions.contains("You are in a side conversation, not the main thread.")
    );
    assert!(
        developer_instructions.contains("Sub-agents are off-limits in this side conversation.")
    );
}

#[tokio::test]
async fn failed_preinstall_cleanup_removes_invisible_side_state() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.ensure_thread_channel(side_thread_id);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    app.fail_side_start(
        &mut tui,
        &mut app_server,
        side_thread_id,
        Some(crate::chatwidget::UserMessage::from("side question")),
        "Failed to install side conversation.".to_string(),
    )
    .await;

    assert!(!app.side_threads.contains_key(&side_thread_id));
    assert!(!app.thread_event_channels.contains_key(&side_thread_id));
    assert!(app.is_thread_retired(&side_thread_id));
    assert!(!app.chat_widget.has_side());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "side question"
    );
    let mut rendered_errors = Vec::new();
    while let Ok(event) = app_event_rx.try_recv() {
        let cell = match event {
            AppEvent::FromConversation { target, event } => {
                assert_eq!(target.pane, PaneSlot::Parent);
                match *event {
                    AppEvent::InsertHistoryCell(cell) => Some(cell),
                    _ => None,
                }
            }
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        };
        if let Some(cell) = cell {
            rendered_errors.push(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .flat_map(|line| &line.spans)
                    .map(|span| span.content.as_ref())
                    .collect::<String>(),
            );
        }
    }
    assert!(
        rendered_errors
            .iter()
            .any(|error| error.contains("Failed to install side conversation.")),
        "startup error should remain visible in Parent: {rendered_errors:?}"
    );
    Ok(())
}
