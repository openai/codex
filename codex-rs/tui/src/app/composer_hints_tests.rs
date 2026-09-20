//! Discovery yields to input and reflects configured shortcuts.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn hints_respect_settings_drafts_and_custom_shortcuts() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.show_tooltips = true;
    app.transcript_cells = vec![Arc::new(crate::history_cell::new_user_prompt(
        "question".into(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))];
    app.keymap.app.find_transcript = vec![crate::key_hint::plain(KeyCode::F(12))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut snapshots = Vec::new();
    for width in [80, 32] {
        let size = Size::new(width, /*height*/ 8);
        tui.terminal.resize(size)?;
        app.render_owned_transcript(&mut tui, size)?;
        let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
        let rendered = (buffer.area.y..buffer.area.bottom())
            .map(|y| {
                (buffer.area.x..buffer.area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        snapshots.push(format!("{width} columns\n{rendered}"));
    }
    insta::assert_snapshot!(
        "composer_tip_layout",
        crate::chatwidget::tests::helpers::normalize_snapshot_paths(snapshots.join("\n\n")),
    );
    app.keymap.app.find_transcript.clear();
    assert_eq!(app.composer_tip(), None);
    app.transcript_cells.clear();
    app.chat_widget.apply_external_edit("draft".into());
    assert_eq!(app.composer_tip(), None);
    app.chat_widget.apply_external_edit(String::new());
    app.local_settings.tui.show_tooltips = false;
    assert_eq!(app.composer_tip(), None);
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn escape_closes_shortcut_help_before_transcript_backtracking() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(
        (0..60)
            .map(|row| format!("response row {row}").into())
            .collect(),
    ))];
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -20);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
    );
    let mut before = ratatui::buffer::Buffer::empty(area);
    app.transcript_view
        .render(area, &mut before, &app.transcript_cells);
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let toggle = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.should_handle_backtrack_esc(escape));
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(toggle))
        .await?;
    assert!(app.chat_widget.shortcut_overlay_visible());
    assert!(!app.should_handle_backtrack_esc(escape));
    assert!(!app.should_reject_side_backtrack_esc(escape));

    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(escape))
        .await?;
    assert!(!app.chat_widget.shortcut_overlay_visible());
    assert!(!app.backtrack.primed);
    assert!(!app.transcript_view.is_following());
    assert!(app.should_handle_backtrack_esc(escape));
    let mut after = ratatui::buffer::Buffer::empty(area);
    app.transcript_view
        .render(area, &mut after, &app.transcript_cells);
    assert_eq!(after, before);

    // Search and selection replace the help footer and keep their own first Escape.
    for search in [true, false] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(toggle))
            .await?;
        if search {
            app.transcript_view.begin_search();
        } else {
            tui.screen_size_for_event(&TuiEvent::Resize(Size::new(
                /*width*/ 80, /*height*/ 40,
            )))?;
            app.handle_owned_transcript_event(
                &mut tui,
                &mut server,
                &TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
            )?;
        }
        assert!(app.transcript_view.has_active_interaction());
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(escape))
            .await?;
        assert!(!app.transcript_view.has_active_interaction());
        assert!(app.chat_widget.shortcut_overlay_visible());
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(escape))
            .await?;
        assert!(!app.chat_widget.shortcut_overlay_visible());
    }
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
