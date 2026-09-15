//! Regression coverage for transcript viewer input, prompt selection, and restoration.
//!
//! The default-off feature must leave the existing viewer and its draft intact.
//! Analytics also preserves the composer and stays separate from transcript backtracking.

use super::*;
use crate::test_support::test_path_display;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn press_key(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    app_server: &mut AppServerSession,
    code: KeyCode,
) -> Result<()> {
    app.handle_tui_event(
        tui,
        app_server,
        TuiEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)),
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn analytics_preserves_the_draft_and_retains_the_view_on_reopen() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget
        .apply_external_edit("preserved draft".into());
    app.handle_event(&mut tui, &mut app_server, AppEvent::OpenAnalytics)
        .await?;
    for code in [
        KeyCode::Char('4'),
        KeyCode::Enter,
        KeyCode::Down,
        KeyCode::Char('r'),
    ] {
        press_key(&mut app, &mut tui, &mut app_server, code).await?;
        assert!(matches!(app.overlay, Some(Overlay::Analytics(_))));
        assert!(!app.backtrack.overlay_preview_active);
    }
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
    assert!(app.overlay.is_none());
    assert!(app.retained_analytics.is_some());
    app.handle_event(&mut tui, &mut app_server, AppEvent::OpenAnalytics)
        .await?;
    assert!(matches!(app.overlay, Some(Overlay::Analytics(_))));
    assert!(app.retained_analytics.is_none());
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "preserved draft"
    );
    Ok(())
}

#[tokio::test]
async fn transcript_flag_off_preserves_viewer_and_backtracking() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let keymap_config = toml::from_str("[composer]\nsubmit = [\"ctrl-x enter\"]")?;
    app.keymap =
        crate::keymap::RuntimeKeymap::from_config(&keymap_config).expect("valid composer chord");
    app.chat_widget
        .apply_keymap_update(keymap_config, &app.keymap);
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session(session);
    app.transcript_cells = ["first", "second"]
        .map(|message| {
            Arc::new(UserHistoryCell {
                message: message.into(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
                spoken: false,
            }) as Arc<dyn HistoryCell>
        })
        .to_vec();
    app.chat_widget
        .apply_external_edit("preserved draft".into());
    app.open_transcript_overlay(&mut tui);
    for event in [
        TuiEvent::Paste("not composer input".into()),
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, event)
            .await?;
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "preserved draft"
    );
    let chord_prefix = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(chord_prefix))
        .await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(!app.backtrack.overlay_preview_active);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 100, /*height*/ 12,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    let Some(Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("viewer closed")
    };
    overlay.render(area, &mut buffer);
    insta::assert_snapshot!("transcript_flag_off_viewer", buffer_text(&buffer));
    for (key, selected) in [
        (KeyCode::Esc, 1),
        (KeyCode::Esc, 0),
        (KeyCode::Right, 1),
        (KeyCode::Right, 1),
    ] {
        press_key(&mut app, &mut tui, &mut app_server, key).await?;
        assert_eq!(app.backtrack.nth_user_message, selected);
    }
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Enter).await?;
    assert!(app.overlay.is_none());
    assert!(
        std::iter::from_fn(|| app_event_rx.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::ForkSessionForPromptEdit {
                nth_user_message: 1,
                ..
            }
        ))
    );
    Ok(())
}

async fn assert_transcript_close_repaints_inline_draft(mut app: App) -> Result<()> {
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.insert_str("EDGE-DRAFT-MUST-SURVIVE");
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    let inline_viewport = tui.terminal.viewport_area;
    app.open_transcript_overlay(&mut tui);
    tui.enter_alt_screen()?;
    app.insert_history_cell(
        &mut tui,
        Box::new(PlainHistoryCell::new(vec!["arrived in transcript".into()])),
    );
    let deferred_history = app.deferred_history_lines.clone();
    assert!(!deferred_history.is_empty());
    app.apply_raw_output_mode(&mut tui, /*enabled*/ true, /*notify*/ false);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert_eq!(app.deferred_history_lines, deferred_history);
    assert!(app.transcript_reflow.has_pending_reflow());

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
    )
    .await?;
    assert_eq!(tui.terminal.viewport_area, inline_viewport);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert!(!app.transcript_reflow.has_pending_reflow());
    assert_eq!(
        app.last_rendered_history_tail
            .as_ref()
            .expect("inline history reflowed")
            .lines,
        deferred_history
    );

    insta::assert_snapshot!(
        "transcript_close_restores_inline_draft",
        buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal
        ))
        .replace(&test_path_display("/tmp/project"), "/tmp/project")
    );
    Ok(())
}

#[tokio::test]
async fn transcript_viewer_close_repaints_preserved_inline_draft() -> Result<()> {
    assert_transcript_close_repaints_inline_draft(make_test_app().await).await
}
