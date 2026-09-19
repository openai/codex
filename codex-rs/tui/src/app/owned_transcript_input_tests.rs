//! Composer input resumes after transcript selection without changing search or keymap ownership.

use super::*;
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::PlainHistoryCell;
use pretty_assertions::assert_eq;

async fn select_transcript(
    app: &mut App,
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    detailed: bool,
) -> Result<()> {
    app.transcript_view = Default::default();
    app.transcript_view
        .set_presentation(detailed, HistoryRenderMode::Rich);
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(vec![
        "replaced cell".into(),
    ]))];
    app.render_owned_transcript(tui, Size::new(/*width*/ 80, /*height*/ 12))?;
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(vec![
        "select this transcript".into(),
    ]))];
    app.handle_tui_event(
        tui,
        app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    )
    .await?;
    assert!(
        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
            .contains("select this transcript")
    );
    for _ in 0..6 {
        app.handle_tui_event(
            tui,
            app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)),
        )
        .await?;
    }
    assert_eq!(
        app.transcript_view
            .selected_text(&app.transcript_cells)
            .as_deref(),
        Some("select")
    );
    Ok(())
}

#[tokio::test]
async fn modified_arrows_and_editor_chords_resume_composer_movement() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let config = serde_json::from_value(serde_json::json!({
        "editor": {"move_line_start": ["home", "ctrl-a", "ctrl-shift-left", "ctrl-x h"]}
    }))?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid editor bindings");
    app.chat_widget.apply_keymap_update(config, &app.keymap);
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    for detailed in [false, true] {
        for (keys, expected) in [
            (
                vec![KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL)],
                "alpha beta Xgamma",
            ),
            (
                vec![KeyEvent::new(KeyCode::Left, KeyModifiers::ALT)],
                "alpha beta Xgamma",
            ),
            (
                vec![
                    KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
                ],
                "alpha beta gammaX",
            ),
            (
                vec![
                    KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
                    KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
                ],
                "alpha beta gammaX",
            ),
            (
                vec![KeyEvent::new(
                    KeyCode::Left,
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                )],
                "Xalpha beta gamma",
            ),
            (
                vec![
                    KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
                ],
                "Xalpha beta gamma",
            ),
            (
                vec![
                    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
                ],
                "aXlpha beta gamma",
            ),
        ] {
            app.chat_widget
                .apply_external_edit("alpha beta gamma".to_string());
            select_transcript(&mut app, &mut tui, &mut app_server, detailed).await?;
            for key in keys {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
                    .await?;
            }
            assert!(!app.transcript_view.has_active_interaction());
            app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Paste("X".to_string()))
                .await?;
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                expected,
                "detailed={detailed}"
            );
        }
    }
    app_server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn composer_paste_releases_selection_and_retains_normal_cursor_movement() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 12);
    for detailed in [false, true] {
        app.chat_widget.apply_external_edit("draft ".to_string());
        select_transcript(&mut app, &mut tui, &mut app_server, detailed).await?;
        app.render_owned_transcript(&mut tui, size)?;
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Paste("café\r\nbeta\rgamma".to_string()),
        )
        .await?;
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "draft café\nbeta\ngamma"
        );
        assert!(!app.transcript_view.has_active_interaction());
        assert!(!app.transcript_view.tick_selection(&app.transcript_cells));
        assert!(!app.handle_owned_transcript_event(
            &mut tui,
            &mut app_server,
            &TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        )?);
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        )
        .await?;
        // A paste inserts immediately, avoiding the terminal's typing-burst timer in this fixture.
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Paste("X".to_string()))
            .await?;
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "draft café\nbeta\ngammXa"
        );
        app.render_owned_transcript(&mut tui, size)?;
    }
    app_server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
