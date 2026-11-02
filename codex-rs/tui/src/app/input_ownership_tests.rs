//! Find and transcript selection retain control keys ahead of configurable chord prefixes.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn selection_copy_cancels_a_pending_find_editor_chord() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.keymap = RuntimeKeymap::from_config(&serde_json::from_value(serde_json::json!({
        "editor": {"move_line_start": ["ctrl-p h", "ctrl-x h"], "move_up": []}
    }))?)
    .expect("valid editor keymap");
    app.transcript_cells = vec![Arc::new(history_cell::PlainHistoryCell::new(vec![
        "needle text".into(),
    ]))];
    app.transcript_view.begin_search();
    app.transcript_view.paste_search("needle");
    for key in [
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        KeyCode::Enter.into(),
        KeyCode::Esc.into(),
    ] {
        assert_eq!(app.route_key_chord_event(&mut tui, key), Some(key));
        assert!(!app.key_chord_matcher.is_pending());
    }
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    assert_eq!(
        app.route_key_chord_event(
            &mut tui,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)
        ),
        None
    );
    assert!(app.key_chord_matcher.is_pending());
    app.transcript_view.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        &app.transcript_cells,
    );
    for _ in "needle text".chars() {
        app.transcript_view
            .handle_key(KeyCode::Right.into(), &app.transcript_cells);
    }
    let copy = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.route_key_chord_event(&mut tui, copy), Some(copy));
    assert!(!app.key_chord_matcher.is_pending());
    assert_eq!(
        app.transcript_view
            .selected_text(&app.transcript_cells)
            .as_deref(),
        Some("needle text")
    );
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn keypress_inspector_distinguishes_ctrl_alt_c_from_ctrl_c() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for (modifier, expected, cancel) in [
        (KeyModifiers::ALT, "ctrl-alt-c", 'c'),
        (KeyModifiers::SHIFT, "ctrl-shift-c", 'C'),
    ] {
        app.chat_widget.open_keymap_debug(&app.keymap);
        let inspect = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL | modifier);
        let inspect = app
            .route_key_chord_event(&mut tui, inspect)
            .expect("physical inspector key");
        app.chat_widget.handle_key_event(inspect);
        let screen =
            crate::chatwidget::tests::render_bottom_popup(&app.chat_widget, /*width*/ 80);
        assert!(
            screen.contains(&format!("Config key: {expected}")),
            "{screen}"
        );
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Char(cancel), KeyModifiers::CONTROL));
        assert!(!app.chat_widget.has_active_view());
    }
    Ok(())
}
