//! The return control preserves composer geometry and draft across mouse navigation and resize.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn follow_control_click_preserves_draft_caret_and_composer_geometry() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.animations = false;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget
        .apply_external_edit("draft stays here".to_string());
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(
        (0..40)
            .map(|row| format!("transcript row {row}").into())
            .collect(),
    ))];
    for (width, running) in [(80, false), (40, false), (80, true)] {
        if running {
            app.chat_widget.handle_server_notification(
                codex_app_server_protocol::ServerNotification::TurnStarted(
                    codex_app_server_protocol::TurnStartedNotification {
                        thread_id: ThreadId::new().to_string(),
                        turn: codex_app_server_protocol::Turn {
                            id: "turn".into(),
                            items_view: codex_app_server_protocol::TurnItemsView::Full,
                            items: Vec::new(),
                            status: codex_app_server_protocol::TurnStatus::InProgress,
                            error: None,
                            started_at: None,
                            completed_at: None,
                            duration_ms: None,
                        },
                    },
                ),
                /*replay_kind*/ None,
            );
        }
        let size = Size::new(width, /*height*/ 12);
        tui.screen_size_for_event(&TuiEvent::Resize(size))?;
        app.transcript_view.jump_to_latest();
        let bottom = app.render_owned_transcript(&mut tui, size)?;
        let cursor = tui.terminal.last_known_cursor_pos;
        if running {
            let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
            insta::assert_snapshot!(
                "running_without_composer_hint",
                crate::chatwidget::tests::helpers::normalize_snapshot_paths(
                    super::tests::buffer_text(buffer)
                ),
            );
        }
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -10);
        tui.screen_size_for_event(&TuiEvent::Resize(size))?;
        app.render_owned_transcript(&mut tui, size)?;
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        for height in [24, 9, 8, 12] {
            let resized = Size::new(width, height);
            tui.screen_size_for_event(&TuiEvent::Resize(resized))?;
            app.render_owned_transcript(&mut tui, resized)?;
            let resized_cursor = tui.terminal.last_known_cursor_pos;
            assert_eq!(
                (
                    app.chat_widget.composer_text_with_pending(),
                    resized_cursor.x,
                    resized.height - resized_cursor.y,
                    app.transcript_view.is_following(),
                ),
                (
                    "draft stays here".to_string(),
                    cursor.x,
                    size.height - cursor.y,
                    false
                )
            );
            if running && height == 9 {
                let buffer =
                    crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
                insta::assert_snapshot!(
                    "running_follow_control_short_terminal",
                    crate::chatwidget::tests::helpers::normalize_snapshot_paths(
                        super::tests::buffer_text(buffer)
                    ),
                );
            }
        }
        let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
        if running {
            insta::assert_snapshot!(
                "running_follow_control",
                crate::chatwidget::tests::helpers::normalize_snapshot_paths(
                    super::tests::buffer_text(buffer)
                ),
            );
        }
        let (column, row) = buffer
            .content()
            .iter()
            .position(|cell| cell.symbol() == "↓")
            .map(|index| buffer.pos_of(index))
            .expect("return control");
        for interrupted in [true, false] {
            for kind in [
                MouseEventKind::Down(MouseButton::Left),
                MouseEventKind::Up(MouseButton::Left),
            ] {
                if interrupted && kind == MouseEventKind::Up(MouseButton::Left) {
                    app.handle_owned_transcript_event(&mut tui, &mut server, &TuiEvent::FocusLost)?;
                    tui.screen_size_for_event(&TuiEvent::Resize(size))?;
                    app.render_owned_transcript(&mut tui, size)?;
                }
                tui.screen_size_for_event(&TuiEvent::Resize(size))?;
                let handled = app.handle_owned_transcript_event(
                    &mut tui,
                    &mut server,
                    &TuiEvent::Mouse(MouseEvent {
                        kind,
                        column,
                        row,
                        modifiers: KeyModifiers::NONE,
                    }),
                )?;
                assert!(handled || interrupted);
            }
            assert_eq!(app.transcript_view.is_following(), !interrupted);
        }
        tui.screen_size_for_event(&TuiEvent::Resize(size))?;
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, bottom);
        assert_eq!(
            (
                app.chat_widget.composer_text_with_pending(),
                tui.terminal.last_known_cursor_pos
            ),
            ("draft stays here".to_string(), cursor)
        );
        assert!(app.chat_widget.queued_user_message_texts().is_empty());
        if running {
            app.transcript_view.show_copy_feedback(
                &Ok(crate::clipboard_copy::CopyStatus::Confirmed),
                /*characters*/ 24,
            );
            let mut snapshots = Vec::new();
            for (width, height) in [(80, 12), (18, 14), (6, 18), (80, 6)] {
                let size = Size::new(width, height);
                tui.screen_size_for_event(&TuiEvent::Resize(size))?;
                app.render_owned_transcript(&mut tui, size)?;
                let buffer =
                    crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
                snapshots.push(format!(
                    "{width} columns, {height} rows\n{}",
                    super::tests::buffer_text(buffer)
                ));
            }
            insta::assert_snapshot!(
                "running_copy_feedback_spacing",
                crate::chatwidget::tests::helpers::normalize_snapshot_paths(snapshots.join("\n\n")),
            );
        }
    }
    tui.set_owned_screen(/*owned*/ false)?;
    assert!(!app.handle_owned_transcript_event(
        &mut tui,
        &mut server,
        &TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 20,
            row: 7,
            modifiers: KeyModifiers::NONE,
        })
    )?);
    server.shutdown().await?;
    Ok(())
}
