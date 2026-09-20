//! Composer input resumes after transcript selection without changing search or keymap ownership.

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
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

fn screen(tui: &tui::Tui) -> String {
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    let rendered = buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    normalize_snapshot_paths(rendered)
}

#[tokio::test]
async fn empty_enter_returns_to_latest_with_contextual_hints() -> Result<()> {
    let (mut app, mut events, mut operations) = make_test_app_with_channels().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    for (width, detailed) in [(80, false), (40, true)] {
        let size = Size::new(width, /*height*/ 12);
        tui.terminal.resize(size)?;
        app.transcript_view = Default::default();
        app.transcript_view
            .set_presentation(detailed, HistoryRenderMode::Rich);
        app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(
            (0..30).map(|row| format!("row {row}").into()).collect(),
        ))];
        app.render_owned_transcript(&mut tui, size)?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -10);
        app.render_owned_transcript(&mut tui, size)?;
        let cursor = tui.terminal.last_known_cursor_pos;
        assert!(screen(&tui).contains("enter/esc latest"));

        // Release/repeat events and modified Enter must not become a navigation command.
        for key in [
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        ] {
            assert!(!app.handle_owned_transcript_event(
                &mut tui,
                &mut server,
                &TuiEvent::Key(key)
            )?);
            assert!(!app.transcript_view.is_following());
        }

        app.chat_widget.apply_external_edit("draft".to_string());
        app.render_owned_transcript(&mut tui, size)?;
        assert!(screen(&tui).contains("esc latest"));
        assert!(!screen(&tui).contains("enter/esc latest"));
        app.chat_widget.apply_external_edit(String::new());
        app.transcript_cells
            .push(Arc::new(PlainHistoryCell::new(vec!["new output".into()])));
        app.render_owned_transcript(&mut tui, size)?;
        while events.try_recv().is_ok() {}
        app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(KeyEvent::from(KeyCode::Enter)),
        )
        .await?;
        app.render_owned_transcript(&mut tui, size)?;
        assert!(app.transcript_view.is_following());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "");
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        assert!(screen(&tui).contains("new output"));
        assert!(!screen(&tui).contains("latest"));
        assert!(events.try_recv().is_err());
        assert!(operations.try_recv().is_err());

        assert!(!app.handle_owned_transcript_event(
            &mut tui,
            &mut server,
            &TuiEvent::Key(KeyEvent::from(KeyCode::Enter))
        )?);
    }
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn submitting_a_draft_from_history_queues_once_and_follows_immediately() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 12);
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(
        (0..30).map(|row| format!("row {row}").into()).collect(),
    ))];
    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.chat_widget
        .apply_external_edit("send this once".to_string());
    while events.try_recv().is_ok() {}

    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::from(KeyCode::Enter)),
    )
    .await?;
    let mut follows = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(event, AppEvent::FollowTranscript) {
            follows += 1;
            app.handle_event(&mut tui, &mut server, event).await?;
        }
    }
    assert_eq!(follows, 1);
    assert!(app.transcript_view.is_following());
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["send this once"]
    );
    assert_eq!(app.chat_widget.composer_text_with_pending(), "");
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn enter_preserves_search_backtrack_and_modal_ownership_while_scrolled() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 12);
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(
        (0..30).map(|row| format!("row {row}").into()).collect(),
    ))];
    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.render_owned_transcript(&mut tui, size)?;
    let enter = TuiEvent::Key(KeyEvent::from(KeyCode::Enter));
    app.transcript_view.begin_search();
    assert!(app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    assert!(app.transcript_view.is_search_active());
    assert!(!app.transcript_view.is_following());
    app.handle_owned_transcript_event(
        &mut tui,
        &mut server,
        &TuiEvent::Key(KeyEvent::from(KeyCode::Esc)),
    )?;

    // An empty selection anchor consumes Enter without accessing the host clipboard.
    app.handle_owned_transcript_event(
        &mut tui,
        &mut server,
        &TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    )?;
    assert!(app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    assert!(!app.transcript_view.is_following());
    app.handle_owned_transcript_event(
        &mut tui,
        &mut server,
        &TuiEvent::Key(KeyEvent::from(KeyCode::Esc)),
    )?;

    app.backtrack.primed = true;
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    app.backtrack.primed = false;
    app.backtrack.overlay_preview_active = true;
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    app.backtrack.overlay_preview_active = false;

    app.chat_widget
        .set_remote_image_urls(vec!["https://example.com/image.png".into()]);
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    app.chat_widget.set_remote_image_urls(Vec::new());
    app.chat_widget.open_feature_enable_prompt(Feature::Collab);
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &enter)?);
    assert!(!app.transcript_view.is_following());

    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn escape_returns_to_latest_without_changing_the_composer_draft() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 12);
    app.chat_widget
        .apply_external_edit("draft preserved".to_string());
    app.transcript_cells = vec![Arc::new(PlainHistoryCell::new(
        (0..30).map(|row| format!("row {row}").into()).collect(),
    ))];
    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.transcript_cells
        .push(Arc::new(PlainHistoryCell::new(vec!["new output".into()])));
    app.render_owned_transcript(&mut tui, size)?;
    let before = screen(&tui);
    assert!(before.contains("New activity · esc latest"));
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    app.render_owned_transcript(&mut tui, size)?;
    assert!(app.transcript_view.is_following());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draft preserved"
    );
    insta::assert_snapshot!(
        "escape_returns_to_latest",
        format!("before\n{before}\n\nafter\n{}", screen(&tui))
    );

    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.render_owned_transcript(&mut tui, size)?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.chat_widget.should_handle_vim_insert_escape(escape));
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(escape))
        .await?;
    assert!(!app.chat_widget.should_handle_vim_insert_escape(escape));
    assert!(!app.transcript_view.is_following());
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(escape))
        .await?;
    assert!(app.transcript_view.is_following());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draft preserved"
    );

    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -10);
    app.transcript_cells
        .push(Arc::new(PlainHistoryCell::new(vec![
            "visible after resize".into(),
        ])));
    let large = Size::new(/*width*/ 80, /*height*/ 24);
    tui.terminal.resize(large)?;
    app.render_owned_transcript(&mut tui, large)?;
    let resized = screen(&tui);
    assert!(resized.contains("visible after resize"));
    assert!(!resized.contains("New activity"));
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
