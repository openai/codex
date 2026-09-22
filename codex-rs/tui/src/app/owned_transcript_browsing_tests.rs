//! Browsing restores its origin and gives selection/search priority over rewind keys.

use super::*;
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::PlainHistoryCell;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn escape_restores_reading_origin_after_details_navigation_and_resize() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![
        user_cell("first prompt"),
        Arc::new(PlainHistoryCell::new(
            (0..30)
                .map(|row| format!("response row {row}").into())
                .collect(),
        )),
        user_cell("second prompt"),
        Arc::new(PlainHistoryCell::new(vec!["second response".into(); 30])),
    ];
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    for detailed in [false, true] {
        app.transcript_view
            .set_presentation(detailed, HistoryRenderMode::Rich);
        app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 16))?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -12);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
        );
        let mut before = Buffer::empty(area);
        app.transcript_view
            .render(area, &mut before, &app.transcript_cells);
        let escape_count = if detailed { 1 } else { 2 };
        for _ in 0..escape_count {
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
                .await?;
        }
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.transcript_view.is_detailed()
            ),
            (true, false)
        );
        for (key, expected_details) in [
            (KeyEvent::from(KeyCode::Left), false),
            (
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
                true,
            ),
            (
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
                false,
            ),
        ] {
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
                .await?;
            assert_eq!(app.transcript_view.is_detailed(), expected_details);
            let mut selected = Buffer::empty(area);
            app.transcript_view
                .render(area, &mut selected, &app.transcript_cells);
            assert!(buffer_text(&selected).contains("first prompt"));
        }
        assert_eq!(app.backtrack.nth_user_message, 0);
        app.render_owned_transcript(&mut tui, Size::new(/*width*/ 40, /*height*/ 12))?;
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
            .await?;
        let mut after = Buffer::empty(area);
        app.transcript_view
            .render(area, &mut after, &app.transcript_cells);
        assert_eq!(buffer_text(&after), buffer_text(&before));
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.transcript_view.is_detailed(),
                app.transcript_view.is_following()
            ),
            (false, detailed, false)
        );
    }
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn browsing_search_and_selection_consume_escape_before_mode_exit() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("first prompt"), user_cell("second prompt")];
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 16))?;
    app.transcript_view.begin_search();
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Paste("first".to_string()))
        .await?;
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert_eq!(
        (
            app.transcript_view.is_search_active(),
            app.backtrack.overlay_preview_active
        ),
        (false, true)
    );
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 16))?;
    for key in [
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
    ] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
            .await?;
    }
    assert!(app.transcript_view.has_active_interaction());
    assert_eq!(app.backtrack.nth_user_message, 1);
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert_eq!(
        (
            app.transcript_view.has_active_interaction(),
            app.backtrack.overlay_preview_active
        ),
        (false, true)
    );
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert!(!app.backtrack.overlay_preview_active);
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn browsing_footer_adapts_to_width() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("first prompt"), user_cell("second prompt")];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    for width in [100, 80, 60, 40, 20, 8] {
        let footer = app
            .prompt_navigation_footer(width)
            .expect("browsing footer");
        assert_eq!(footer.text.lines.len(), 1);
        assert!(footer.text.width() <= usize::from(width));
    }
    app.transcript_view.history = TranscriptHistoryState::Failed;
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 12))?;
    assert!(
        buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal,
        ))
        .contains("Retry history")
    );
    app.transcript_view.history = TranscriptHistoryState::Complete;
    app.cancel_transcript_browsing(&mut tui);
    tui.set_owned_screen(/*owned*/ false)?;
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    let mut snapshots = Vec::new();
    for width in [80, 32] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 8);
        let footer = app
            .prompt_navigation_footer(width.saturating_sub(/*rhs*/ 2))
            .expect("browsing footer");
        let Some(Overlay::Transcript(overlay)) = &mut app.overlay else {
            panic!("expected inline browsing");
        };
        overlay.browsing_footer = footer.text.lines.into_iter().next();
        let mut buffer = Buffer::empty(area);
        overlay.render(area, &mut buffer);
        snapshots.push(format!("{width} columns\n{}", buffer_text(&buffer)));
    }
    insta::assert_snapshot!("browsing_footer", snapshots.join("\n\n"));
    let Some(Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("expected inline browsing");
    };
    overlay.set_history_state(TranscriptHistoryState::Failed);
    let footer = app.prompt_navigation_footer(/*width*/ 78);
    let Some(Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("expected inline browsing");
    };
    overlay.browsing_footer = footer.and_then(|footer| footer.text.lines.into_iter().next());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    assert!(buffer_text(&buffer).contains("Retry history"));
    app.close_transcript_overlay(&mut tui);
    Ok(())
}

#[tokio::test]
async fn inline_browsing_is_compact_and_escape_restores_the_existing_overlay() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("first prompt"), user_cell("second prompt")];
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.open_transcript_overlay(&mut tui);
    for (key, expected_details, browsing) in [
        (KeyEvent::from(KeyCode::Esc), false, true),
        (
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            true,
            true,
        ),
        (KeyEvent::from(KeyCode::Esc), true, false),
    ] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
            .await?;
        let Some(Overlay::Transcript(overlay)) = &app.overlay else {
            panic!("overlay must remain open")
        };
        assert_eq!(
            (overlay.is_detailed(), app.backtrack.overlay_preview_active),
            (expected_details, browsing)
        );
    }
    app.close_transcript_overlay(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert!(app.overlay.is_none());
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn browsing_details_use_the_remapped_chord_without_cancelling_preview() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![
        user_cell("first prompt"),
        user_cell(&"second prompt\n".repeat(80)),
    ];
    let config = toml::from_str(
        "[global]\nopen_transcript = [\"ctrl-x ctrl-t\", \"left f6\", \"enter f6\"]\ncopy = [\"ctrl-x ctrl-p\"]\n[pager]\nclose_transcript = [\"ctrl-x ctrl-t\"]\nscroll_up = []\nclose = [\"up ctrl-x\"]\npage_down = [\"ctrl-x ctrl-p\"]\njump_top = [\"ctrl-x ctrl-h\"]\n[composer]\nsubmit = []\n[editor]\nmove_left = []\ninsert_newline = []\n[vim_normal]\nmove_left = []",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid transcript chord");
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for owned in [true, false] {
        tui.set_owned_screen(owned)?;
        app.handle_backtrack_esc_key(&mut tui);
        app.handle_backtrack_esc_key(&mut tui);
        assert!(
            app.prompt_navigation_footer(/*width*/ 100)
                .expect("footer")
                .text
                .to_string()
                .contains("ctrl+x ctrl+t")
        );
        for (keys, expected_details) in [
            (
                [
                    KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
                ],
                true,
            ),
            ([KeyCode::Left.into(), KeyCode::F(6).into()], false),
            ([KeyCode::Enter.into(), KeyCode::F(6).into()], true),
        ] {
            for key in keys {
                app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
                    .await?;
                assert!(app.backtrack.overlay_preview_active);
            }
            let detailed = match &app.overlay {
                Some(Overlay::Transcript(overlay)) => overlay.is_detailed(),
                _ => app.transcript_view.is_detailed(),
            };
            assert_eq!(detailed, expected_details);
        }
        for completion in ['h', 'p'] {
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)
                .await?;
            for code in ['x', completion] {
                app.handle_tui_event(
                    &mut tui,
                    &mut server,
                    TuiEvent::Key(KeyEvent::new(KeyCode::Char(code), KeyModifiers::CONTROL)),
                )
                .await?;
                if code == 'x' {
                    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)
                        .await?;
                    let text = buffer_text(
                        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
                    );
                    assert!(text.contains("page down"), "pending pager chord is visible");
                }
            }
        }
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)
            .await?;
        let text = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal,
        ));
        assert!(
            !text.contains("first prompt"),
            "configured page-down moved the viewport"
        );
        assert!(text.contains("second prompt"));
        assert!(app.backtrack.overlay_preview_active);
        for code in [KeyCode::Char('h'), KeyCode::Right] {
            if let Some(Overlay::Transcript(overlay)) = &mut app.overlay {
                overlay.set_history_state(TranscriptHistoryState::LoadingBeginning);
            } else {
                app.transcript_view.history = TranscriptHistoryState::LoadingBeginning;
            }
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(code.into()))
                .await?;
            let state = match &app.overlay {
                Some(Overlay::Transcript(overlay)) => overlay.history_state(),
                _ => app.transcript_view.history,
            };
            assert_eq!(state, TranscriptHistoryState::LoadingOlder);
        }
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Up.into()))
            .await?;
        assert!(app.backtrack.overlay_preview_active);
        app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        )
        .await?;
        assert!(!app.backtrack.overlay_preview_active);
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn browsing_requires_fresh_escape_presses_and_ignores_confirmation_repeats() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("first prompt"), user_cell("second prompt")];
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.transcript_view
        .set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
    app.chat_widget.apply_external_edit("draft".to_string());
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert!(!app.backtrack.overlay_preview_active);
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
    app.chat_widget.apply_external_edit(String::new());
    app.transcript_view
        .set_presentation(/*detailed*/ false, HistoryRenderMode::Rich);
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
    )
    .await?;
    assert!(!app.backtrack.overlay_preview_active);
    for key in [KeyCode::PageUp, KeyCode::F(8)] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))
            .await?;
        assert!(!app.backtrack.primed);
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
            .await?;
        assert!(!app.backtrack.overlay_preview_active);
    }
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Moved,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }),
    )
    .await?;
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    for (code, kind) in [
        (KeyCode::Esc, KeyEventKind::Repeat),
        (KeyCode::Enter, KeyEventKind::Repeat),
        (KeyCode::Enter, KeyEventKind::Release),
    ] {
        app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)),
        )
        .await?;
        assert!(app.backtrack.overlay_preview_active);
    }
    // Enter during history loading has no selected prompt to confirm.
    app.backtrack.nth_user_message = usize::MAX;
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into()))
        .await?;
    assert!(app.backtrack.overlay_preview_active);
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::RevertSessionForPromptEdit { .. }))
    );
    for (binding, keys) in [
        ("f8", vec![KeyCode::F(8)]),
        ("f8 v", vec![KeyCode::F(8), KeyCode::Char('v')]),
    ] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(&format!(
            "[chat]\ntoggle_voice = '{binding}'"
        ))?)
        .expect("valid voice binding");
        let thread_id = app.chat_widget.thread_id().unwrap();
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
        for key in keys {
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))
                .await?;
        }
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.chat_widget.realtime_conversation_is_running()
            ),
            (false, false)
        );
        for _ in 0..2 {
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
                .await?;
        }
        assert!(app.backtrack.overlay_preview_active);
    }
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[global]\nopen_external_editor = [\"f6 f7\"]",
    )?)
    .expect("valid editor chord");
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(6).into()))
        .await?;
    assert!(!app.backtrack.overlay_preview_active);
    assert!(app.key_chord_matcher.is_pending());
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(7).into()))
        .await?;
    assert_eq!(
        (
            app.backtrack.overlay_preview_active,
            app.chat_widget.external_editor_state()
        ),
        (false, ExternalEditorState::Requested)
    );
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn browsing_arrows_and_vim_keys_navigate_without_editing_the_draft() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![
        user_cell("first prompt"),
        Arc::new(PlainHistoryCell::new(
            (0..30)
                .map(|row| format!("response row {row}").into())
                .collect(),
        )),
        user_cell("second prompt"),
    ];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.handle_backtrack_esc_key(&mut tui);
    app.handle_backtrack_esc_key(&mut tui);
    app.chat_widget
        .apply_external_edit("untouched draft".to_string());
    let size = Size::new(/*width*/ 80, /*height*/ 16);
    app.render_owned_transcript(&mut tui, size)?;
    let before = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
        &tui.terminal,
    ));
    for code in [KeyCode::Up, KeyCode::Char('k')] {
        assert!(app.handle_owned_backtrack_event(&mut tui, &TuiEvent::Key(code.into()))?);
    }
    app.render_owned_transcript(&mut tui, size)?;
    pretty_assertions::assert_ne!(
        buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal
        )),
        before
    );
    for (code, selected) in [
        (KeyCode::Down, 1),
        (KeyCode::Char('j'), 1),
        (KeyCode::Char('h'), 0),
        (KeyCode::Char('l'), 1),
        (KeyCode::Left, 0),
        (KeyCode::Right, 1),
    ] {
        assert!(app.handle_owned_backtrack_event(&mut tui, &TuiEvent::Key(code.into()))?);
        assert_eq!(app.backtrack.nth_user_message, selected);
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "untouched draft"
    );
    assert!(app.backtrack.overlay_preview_active);
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
