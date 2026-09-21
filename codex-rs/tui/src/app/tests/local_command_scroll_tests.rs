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
async fn settings_pickers_preserve_the_reading_anchor_through_open_and_close() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget.handle_thread_session(test_thread_session(
        ThreadId::new(),
        app.config.cwd.to_path_buf(),
    ));
    app.sync_tui_theme_selection("base16-ocean-dark".to_string());
    hold_older_history(&mut app, &mut tui);
    let held = transcript_buffer(&mut app);

    for (command, close) in [
        ("/model", KeyCode::Esc),
        ("/theme", KeyCode::Enter),
        ("/keymap", KeyCode::Esc),
        ("/memories", KeyCode::Esc),
        ("/title", KeyCode::Esc),
        ("/statusline", KeyCode::Esc),
    ] {
        while events.try_recv().is_ok() {}
        submit_local_command(&mut app, command);
        while let Ok(event) = events.try_recv() {
            app.handle_event(&mut tui, &mut server, event).await?;
        }
        assert!(app.chat_widget.has_active_view(), "{command}");
        assert!(!app.transcript_view.is_following(), "{command} opened");
        assert_eq!(transcript_buffer(&mut app), held, "{command} opened");

        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(close.into()))
            .await?;
        let mut selected_theme = None;
        while let Ok(event) = events.try_recv() {
            if let AppEvent::SyntaxThemeSelected { name } = &event {
                selected_theme = Some(name.clone());
            }
            app.handle_event(&mut tui, &mut server, event).await?;
        }
        if command == "/theme" {
            assert_eq!(selected_theme, Some("base16-ocean-dark".to_string()));
        }
        assert!(!app.chat_widget.has_active_view(), "{command}");
        assert!(!app.transcript_view.is_following(), "{command} closed");
        assert_eq!(transcript_buffer(&mut app), held, "{command} closed");
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
    older 0
    older 1
    older 2
    ");
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn settings_picker_save_failures_reveal_their_errors() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.sync_tui_theme_selection("base16-ocean-dark".to_string());
    // Break writes only after startup, as can happen when the config is edited externally.
    std::fs::write(app.local_settings.user_config_path.as_path(), "[broken")?;

    for event in [
        AppEvent::SyntaxThemeSelected {
            name: "base16-ocean-dark".into(),
        },
        AppEvent::StatusLineSetup {
            items: vec![],
            use_theme_colors: false,
        },
        AppEvent::TerminalTitleSetup { items: vec![] },
        AppEvent::PersistModelSelection {
            model: "gpt-5.5".into(),
            effort: Some(ReasoningEffortConfig::High),
        },
        AppEvent::ApplyAdvancedReasoning {
            model: "gpt-5.5".into(),
            effort: ReasoningEffortConfig::Ultra,
        },
        AppEvent::PersistPlanModeReasoningEffort(Some(ReasoningEffortConfig::High)),
        AppEvent::UpdateMemorySettings {
            use_memories: true,
            generate_memories: true,
        },
        AppEvent::EnableFeatureForNewThreads(Feature::MemoryTool),
        AppEvent::KeymapCaptured {
            context: "global".into(),
            action: "find_transcript".into(),
            key: "f12".into(),
            intent: crate::app_event::KeymapEditIntent::ReplaceAll,
        },
        AppEvent::KeymapCleared {
            context: "global".into(),
            action: "find_transcript".into(),
        },
    ] {
        hold_older_history(&mut app, &mut tui);
        while events.try_recv().is_ok() {}
        let selection = format!("{event:?}");
        let is_theme = matches!(event, AppEvent::SyntaxThemeSelected { .. });
        if is_theme {
            submit_local_command(&mut app, "/theme");
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into()))
                .await?;
        } else {
            app.handle_event(&mut tui, &mut server, event).await?;
        }
        while let Ok(event) = events.try_recv() {
            app.handle_event(&mut tui, &mut server, event).await?;
        }
        assert!(app.transcript_view.is_following(), "{selection}");
        let error = app.transcript_cells.last().expect("save error");
        assert!(
            lines_to_single_string(&error.display_lines(/*width*/ 80)).contains("Failed to"),
            "{selection}"
        );
        if is_theme {
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
            );
            let mut buffer = Buffer::empty(area);
            app.transcript_view
                .render(area, &mut buffer, &app.transcript_cells);
            let visible = buffer
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
            older 4
            older 5

            ■ Failed to save theme: TOML parse error at line 1, column 8
              |
            1 | [broken
              |        ^
            unclosed table, expected `]`
            ");
        }
    }
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn settings_confirmations_reveal_their_history_output() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;
    // A stale thread id makes the active-thread update fail independently of config writes.
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    for (event, output) in [
        (
            AppEvent::SelectSessionModel {
                model: "gpt-5.5".into(),
                effort: Some(ReasoningEffortConfig::High),
            },
            "Failed to update thread settings",
        ),
        (
            AppEvent::UpdateLunaReserveReasoning {
                thread_id,
                effort: Some(ReasoningEffortConfig::High),
            },
            "Failed to update thread settings",
        ),
        (AppEvent::ResetMemories, "Reset local memories."),
        (
            AppEvent::KeymapCaptured {
                context: "global".into(),
                action: "find_transcript".into(),
                key: "f12".into(),
                intent: crate::app_event::KeymapEditIntent::ReplaceAll,
            },
            "global.find_transcript",
        ),
        (
            AppEvent::KeymapCleared {
                context: "global".into(),
                action: "find_transcript".into(),
            },
            "Removed custom shortcut",
        ),
    ] {
        hold_older_history(&mut app, &mut tui);
        while events.try_recv().is_ok() {}
        if matches!(event, AppEvent::UpdateLunaReserveReasoning { .. }) {
            app.chat_widget
                .set_model(crate::model_catalog::LUNA_RESERVE_MODEL);
        }
        let previous_cells = app.transcript_cells.len();
        app.handle_event(&mut tui, &mut server, event).await?;
        while let Ok(event) = events.try_recv() {
            app.handle_event(&mut tui, &mut server, event).await?;
        }
        assert!(app.transcript_view.is_following());
        let messages = app.transcript_cells[previous_cells..]
            .iter()
            .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 80)))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(messages.contains(output), "expected {output:?}: {messages}");
    }
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn local_command_and_inline_error_reveal_their_history_output() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = start_config_write_test_app_server(&app).await?;
    tui.set_owned_screen(/*owned*/ true)?;

    for (command, output) in [
        (
            "/model",
            "Model selection is disabled until startup completes.",
        ),
        ("/status", "/status"),
        ("/keymap invalid", "Usage: /keymap [debug]"),
    ] {
        hold_older_history(&mut app, &mut tui);
        while events.try_recv().is_ok() {}
        submit_local_command(&mut app, command);
        let mut inserted = false;
        while let Ok(event) = events.try_recv() {
            if matches!(event, AppEvent::InsertHistoryCell(_)) {
                inserted = true;
            }
            app.handle_event(&mut tui, &mut server, event).await?;
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
