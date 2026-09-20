//! Explicit local commands return to their output without making background updates
//! steal the reader's position in the owned transcript.

use super::*;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;

fn hold_older_history(app: &mut App, tui: &mut crate::tui::Tui) {
    app.insert_history_cell(
        tui,
        Box::new(PlainHistoryCell::new(
            (0..6)
                .map(|index| Line::from(format!("older {index}")))
                .collect(),
        )),
    );
    app.transcript_view
        .jump_to_entry(&app.transcript_cells, /*index*/ 0);
    assert!(!app.transcript_view.is_following());
}

fn transcript_buffer(app: &mut App) -> Buffer {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
    );
    let mut buffer = Buffer::empty(area);
    app.transcript_view
        .render(area, &mut buffer, &app.transcript_cells);
    buffer
}

fn submit_local_command(app: &mut App, command: &str) {
    app.chat_widget.apply_external_edit(command.to_owned());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    // Closing the slash popup can require one Enter before submitting. Stop as
    // soon as the command is consumed so a newly opened picker stays untouched.
    for _ in 0..2 {
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        if app.chat_widget.composer_text_with_pending().is_empty() {
            break;
        }
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "");
}

#[tokio::test]
async fn local_command_and_inline_error_reveal_their_history_output() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;

    for (command, output) in [
        ("/status", "/status"),
        ("/keymap invalid", "Usage: /keymap [debug]"),
    ] {
        hold_older_history(&mut app, &mut tui);
        while events.try_recv().is_ok() {}
        submit_local_command(&mut app, command);
        let follow = events.try_recv().expect("command follow event");
        assert_matches!(&follow, AppEvent::FollowTranscript);
        app.handle_event(&mut tui, &mut server, follow).await?;
        let mut inserted = false;
        while let Ok(event) = events.try_recv() {
            if matches!(&event, AppEvent::InsertHistoryCell(_)) {
                app.handle_event(&mut tui, &mut server, event).await?;
                inserted = true;
            }
        }
        assert!(inserted, "{command} should emit local history output");
        assert!(app.transcript_view.is_following());
        let cell = app.transcript_cells.last().expect("local command output");
        assert!(lines_to_single_string(&cell.display_lines(/*width*/ 80)).contains(output));
    }

    let visible = transcript_buffer(&mut app)
        .content
        .chunks(/*chunk_size*/ 80)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(visible, @"
    older 5

    ■ Usage: /keymap [debug]
    ");
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn copy_shortcut_reveals_its_feedback_without_changing_the_draft() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    hold_older_history(&mut app, &mut tui);
    app.chat_widget
        .apply_external_edit("draft preserved".to_string());
    while events.try_recv().is_ok() {}

    // No response avoids the host clipboard while exercising the real shortcut route.
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
    )
    .await?;
    assert!(!app.transcript_view.is_following());
    // The default copy shortcut is a chord; its prefix alone must not move the viewport.
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('o').into()),
    )
    .await?;
    while let Ok(event) = events.try_recv() {
        app.handle_event(&mut tui, &mut server, event).await?;
    }

    assert!(app.transcript_view.is_following());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draft preserved"
    );
    let visible = transcript_buffer(&mut app)
        .content
        .chunks(/*chunk_size*/ 80)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(visible, @"
    older 5

    ■ No agent response to copy
    ");
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn dynamic_service_tier_command_returns_to_latest() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    set_fast_mode_test_catalog(&mut app.chat_widget);
    app.chat_widget.set_model("gpt-5.4");
    app.chat_widget
        .set_feature_enabled(Feature::FastMode, /*enabled*/ true);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    hold_older_history(&mut app, &mut tui);
    while events.try_recv().is_ok() {}

    submit_local_command(&mut app, "/fast");

    let follow = events.try_recv().expect("service tier follow event");
    assert_matches!(&follow, AppEvent::FollowTranscript);
    app.handle_event(&mut tui, &mut server, follow).await?;
    assert!(app.transcript_view.is_following());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::PersistServiceTierSelection { service_tier: Some(tier) }
                if tier == ServiceTier::Fast.request_value()
        ))
    );

    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn usage_picker_opens_analytics_without_moving_the_background_transcript() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    set_chatgpt_auth(&mut app.chat_widget);
    let startup_request = app.chat_widget.start_rate_limit_reset_startup_check();
    assert!(app.chat_widget.finish_rate_limit_reset_hint_refresh(
        startup_request,
        Vec::new(),
        Ok(RateLimitResetCreditsSummary {
            available_count: 1,
            credits: None,
        }),
    ));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    hold_older_history(&mut app, &mut tui);
    while events.try_recv().is_ok() {}

    submit_local_command(&mut app, "/usage");
    let follow = events.try_recv().expect("usage command follow event");
    assert_matches!(&follow, AppEvent::FollowTranscript);
    app.handle_event(&mut tui, &mut server, follow).await?;
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("View analytics"));
    while events.try_recv().is_ok() {}
    app.transcript_view
        .jump_to_entry(&app.transcript_cells, /*index*/ 0);

    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let open = events.try_recv().expect("usage picker action");
    assert_matches!(&open, AppEvent::OpenAnalytics { view: None });
    let held = transcript_buffer(&mut app);
    app.handle_event(&mut tui, &mut server, open).await?;
    assert!(matches!(app.overlay.as_ref(), Some(Overlay::Analytics(_))));
    assert_eq!(transcript_buffer(&mut app), held);
    assert!(!app.transcript_view.is_following());

    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn background_history_insertion_keeps_the_visible_reading_rows() -> Result<()> {
    let (mut app, _events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    hold_older_history(&mut app, &mut tui);
    let held = transcript_buffer(&mut app);

    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::InsertHistoryCell(Box::new(PlainHistoryCell::new(vec![
            "background task finished".into(),
        ]))),
    )
    .await?;

    assert_eq!(transcript_buffer(&mut app), held);
    assert!(!app.transcript_view.is_following());
    assert_eq!(app.transcript_cells.len(), 2);
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}
