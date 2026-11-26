#![cfg(test)]

use super::key_chords::press;
use super::*;
use crate::app::sidebar::PaneFocus;
use crate::app::sidebar::SidebarContent;
use crossterm::event::MouseEvent;
use pretty_assertions::assert_eq;
use unicode_width::UnicodeWidthStr;

async fn file_path_test_app() -> Result<(Box<App>, tui::Tui, AppServerSession)> {
    let mut app = make_test_app().await;
    let parent = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(parent, app.config.cwd.to_path_buf()),
        vec![test_turn("parent-turn", TurnStatus::Completed, Vec::new())],
    )
    .await?;
    app.transcript_cells
        .push(Arc::new(crate::history_cell::AgentMarkdownCell::new(
            "[Guide](./guide.md)".into(),
            app.config.cwd.as_path(),
        )));
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut server = start_config_write_test_app_server(&app).await?;
    app.refresh_sidebar_files(&server, parent).await;
    let pane = app.sidebar_mut();
    pane.show(SidebarContent::Chooser, PaneFocus::Sidebar);
    pane.choice = pane.chat_tabs().file_start();
    let path = KeyCode::Char('p').into();
    Box::pin(press(&mut app, &mut tui, &mut server, path)).await?;
    assert!(app.sidebar().unwrap().showing_file_path);
    Ok((app, tui, server))
}

#[tokio::test]
async fn sidebar_file_path_escape_requires_focus_but_mouse_back_does_not() -> Result<()> {
    let (mut app, mut tui, mut server) = file_path_test_app().await?;
    let size = Size::new(/*width*/ 140, /*height*/ 18);
    let choice = app.sidebar().unwrap().choice;
    let path = KeyCode::Char('p').into();
    let esc = KeyCode::Esc.into();
    app.focus_workspace_composer(&mut tui);
    let frame = side_frame(&mut app, &mut tui, size)?;
    fn footer(frame: &str) -> &str {
        let row = frame.lines().last().unwrap();
        row.rsplit_once('│').unwrap().1.trim()
    }
    insta::assert_snapshot!(footer(&frame), @"back · ⌃x s focus sidebar");
    Box::pin(press(&mut app, &mut tui, &mut server, esc)).await?;
    assert!(app.backtrack.primed);
    assert_eq!(app.chooser_focus(), Some(PaneFocus::Main));
    assert!(app.sidebar().unwrap().showing_file_path);

    let click_back =
        async |app: &mut App, tui: &mut tui::Tui, server: &mut AppServerSession| -> Result<()> {
            let frame = side_frame(app, tui, size)?;
            let (prefix, _) = frame.lines().last().unwrap().rsplit_once("back").unwrap();
            let left = MouseButton::Left;
            for kind in [MouseEventKind::Down(left), MouseEventKind::Up(left)] {
                let event = mouse(kind, prefix.width() as u16, size.height - 1);
                boxed_future!(app.handle_tui_event(tui, server, event)).await?;
            }
            assert!(!app.sidebar().unwrap().showing_file_path);
            assert_eq!(app.chooser_focus(), Some(PaneFocus::Sidebar));
            assert_eq!(app.sidebar().unwrap().choice, choice);
            Ok(())
        };
    click_back(&mut app, &mut tui, &mut server).await?;

    // Click below the file list to resume keyboard navigation on the selected file.
    app.focus_workspace_composer(&mut tui);
    side_frame(&mut app, &mut tui, size)?;
    let panel = app.conversation_frame.panel_area().unwrap();
    let left = MouseButton::Left;
    for kind in [MouseEventKind::Down(left), MouseEventKind::Up(left)] {
        let event = mouse(kind, panel.right() - 1, panel.bottom() - 2);
        boxed_future!(app.handle_tui_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.chooser_focus(), Some(PaneFocus::Sidebar));
    Box::pin(press(&mut app, &mut tui, &mut server, path)).await?;
    let frame = side_frame(&mut app, &mut tui, size)?;
    insta::assert_snapshot!(footer(&frame), @"esc back · ↓ scroll");
    click_back(&mut app, &mut tui, &mut server).await?;

    app.local_settings.tui.keymap = toml::from_str("[list]\ncancel = []")?;
    enable_side_retention(&mut app)?;
    Box::pin(press(&mut app, &mut tui, &mut server, path)).await?;
    let frame = side_frame(&mut app, &mut tui, size)?;
    assert_eq!(footer(&frame), "back · ↓ scroll");
    click_back(&mut app, &mut tui, &mut server).await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn sidebar_file_path_body_click_focuses_without_bypassing_pending_chord() -> Result<()> {
    let (mut app, mut tui, mut server) = file_path_test_app().await?;
    app.chat_widget.insert_str("parent draft");
    let choice = app.sidebar().unwrap().choice;
    app.focus_workspace_composer(&mut tui);
    let size = Size::new(/*width*/ 140, /*height*/ 18);
    side_frame(&mut app, &mut tui, size)?;
    let body =
        crate::sidebar_chooser::file_path_areas(app.conversation_frame.panel_area().unwrap())[0];
    let point = (body.right() - 1, body.bottom() - 1);
    for (button, modifiers) in [
        (MouseButton::Left, KeyModifiers::SHIFT),
        (MouseButton::Left, KeyModifiers::CONTROL),
        (MouseButton::Middle, KeyModifiers::NONE),
        (MouseButton::Left, KeyModifiers::NONE),
    ] {
        for kind in [MouseEventKind::Down(button), MouseEventKind::Up(button)] {
            let event = TuiEvent::Mouse(MouseEvent {
                kind,
                column: point.0,
                row: point.1,
                modifiers,
            });
            boxed_future!(app.handle_tui_event(&mut tui, &mut server, event)).await?;
        }
        let focus = if button == MouseButton::Left && modifiers.is_empty() {
            PaneFocus::Sidebar
        } else {
            PaneFocus::Main
        };
        assert_eq!(app.chooser_focus(), Some(focus));
        assert_eq!(app.sidebar().unwrap().choice, choice);
        assert!(app.sidebar().unwrap().showing_file_path);
        assert_eq!(app.chat_widget.composer_text_with_pending(), "parent draft");
        side_frame(&mut app, &mut tui, size)?;
    }

    let prefix = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    let esc = KeyCode::Esc.into();
    Box::pin(press(&mut app, &mut tui, &mut server, prefix)).await?;
    assert!(app.key_chord_matcher.is_pending());
    Box::pin(press(&mut app, &mut tui, &mut server, esc)).await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(app.sidebar().unwrap().showing_file_path);
    Box::pin(press(&mut app, &mut tui, &mut server, esc)).await?;
    assert!(!app.sidebar().unwrap().showing_file_path);
    assert_eq!(app.chat_widget.composer_text_with_pending(), "parent draft");
    server.shutdown().await?;
    Ok(())
}
