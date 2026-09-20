//! Exercise warning delivery, detailed transcript toggling, and clearing through the owned UI.

use super::*;
use pretty_assertions::assert_eq;

fn screen(tui: &tui::Tui) -> String {
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn warning_notice_keeps_details_in_transcript_and_preserves_draft() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    app.chat_widget
        .apply_external_edit("draft stays here".into());
    let before = app.render_owned_transcript(&mut tui, size)?;
    let cursor = tui.terminal.last_known_cursor_pos;
    for message in [
        "MCP example handshake failed",
        "MCP startup incomplete (example)",
    ] {
        app.insert_history_cell(
            &mut tui,
            Box::new(history_cell::StartupWarningsCell::mcp(
                vec![message.into()],
                ["example".into()],
                /*failure_reason*/ None,
            )),
        );
    }
    for _ in 0..2 {
        app.insert_history_cell(
            &mut tui,
            Box::new(history_cell::new_warning_event(
                "Sample runtime warning".into(),
            )),
        );
    }
    assert_eq!(app.render_owned_transcript(&mut tui, size)?, before);
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
    let live = screen(&tui);
    assert!(live.contains("⚠ 2 warnings"));
    let shortcut = app
        .keymap
        .primary_hint(crate::keymap::KeymapContext::Global, "open_warnings")
        .unwrap()
        .display_label();
    assert!(live.contains(&format!("{shortcut} to view")));
    assert!(!live.contains("handshake failed"));
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    let position = buffer
        .area
        .positions()
        .find(|position| buffer[*position].symbol() == "⚠")
        .expect("warning badge");
    assert!(app.chat_widget.handle_warning_event(
        &TuiEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: position.x,
            row: position.y,
            modifiers: KeyModifiers::NONE,
        }),
        &app.transcript_cells
    ));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.open_transcript_overlay(&mut tui);
    app.render_owned_transcript(&mut tui, size)?;
    let detailed = screen(&tui);
    assert!(detailed.contains("MCP example handshake failed"));
    assert!(detailed.contains("Sample runtime warning"));
    assert!(app.handle_owned_backtrack_event(
        &mut tui,
        &TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
    )?);
    app.render_owned_transcript(&mut tui, size)?;
    assert_eq!(screen(&tui), live);
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
    app.reset_transcript_state_after_clear();
    app.render_owned_transcript(&mut tui, size)?;
    assert!(!screen(&tui).contains("warnings"));
    assert_eq!(history_cell::warning_count(&app.transcript_cells), 0);
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
