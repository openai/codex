//! Usage warnings preserve composer geometry and yield to active interactions.

use super::*;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use pretty_assertions::assert_eq;

fn quota(used_percent: i32) -> RateLimitSnapshot {
    serde_json::from_value(serde_json::json!({
        "primary": {"usedPercent": used_percent, "windowDurationMins": 300},
    }))
    .unwrap()
}

#[tokio::test]
async fn usage_notice_preserves_composer_geometry_on_recovery() -> Result<()> {
    let mut snapshots = Vec::new();
    for (width, height, running, queued, used_percent) in [
        (80, 10, false, false, 92),
        (80, 12, true, false, 98),
        (32, 10, true, false, 101),
        (18, 10, true, false, 95),
        (80, 18, true, true, 98),
        (80, 11, true, true, 98),
        (80, 6, true, false, 98),
    ] {
        let (mut app, mut events, _ops) = crate::app::tests::make_test_app_with_channels().await;
        app.local_settings.tui.show_tooltips = !running;
        app.local_settings.tui.animations = false;
        app.transcript_cells
            .push(Arc::new(crate::history_cell::PlainHistoryCell::new(
                if running {
                    (0..20)
                        .map(|row| format!("Conversation row {row}").into())
                        .collect()
                } else {
                    vec!["Conversation".into()]
                },
            )));
        if running {
            app.chat_widget.handle_server_notification(
                ServerNotification::TurnStarted(TurnStartedNotification {
                    thread_id: ThreadId::new().to_string(),
                    turn: Turn {
                        id: "turn".into(),
                        items_view: TurnItemsView::Full,
                        items: Vec::new(),
                        status: TurnStatus::InProgress,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                }),
                /*replay_kind*/ None,
            );
            assert!(app.chat_widget.is_user_turn_pending_or_running());
            if queued {
                app.keymap.chat.edit_queued_message = vec![crate::key_hint::shift(KeyCode::Left)];
                app.chat_widget
                    .apply_keymap_update(Default::default(), &app.keymap);
                app.chat_widget
                    .apply_external_edit("queued follow-up".into());
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
                assert_eq!(
                    app.chat_widget.queued_user_message_texts(),
                    vec!["queued follow-up"],
                );
            }
            app.chat_widget.apply_external_edit(if queued {
                "draft stays\nsecond line".into()
            } else {
                "draft stays".into()
            });
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        let size = Size::new(width, height);
        tui.terminal.resize(size)?;
        let before = app.render_owned_transcript(&mut tui, size)?;
        let cursor = tui.terminal.last_known_cursor_pos;
        let hint = app.composer_hint(width);
        app.chat_widget
            .on_rate_limit_snapshot(Some(quota(used_percent)));
        while let Ok(event) = events.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                app.insert_history_cell(&mut tui, cell);
            }
        }
        app.render_owned_transcript(&mut tui, size)?;
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
        let screen = buffer
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
        snapshots.push(format!(
            "{width} columns, {height} rows, running={running}, queued={queued}\n{screen}"
        ));
        app.chat_widget
            .on_rate_limit_snapshot(Some(quota(/*used_percent*/ 10)));
        assert_eq!(app.composer_hint(width), hint);
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, before);
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        tui.set_owned_screen(/*owned*/ false)?;
    }
    insta::assert_snapshot!(
        "composer_usage_notice",
        crate::chatwidget::tests::helpers::normalize_snapshot_paths(snapshots.join("\n\n")),
    );
    Ok(())
}

#[tokio::test]
async fn usage_notice_yields_to_interactions_and_blocking_banners() {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.show_tooltips = true;
    let hint = app.composer_hint(/*width*/ 80);
    app.chat_widget
        .on_rate_limit_snapshot(Some(quota(/*used_percent*/ 92)));
    let notice = app.composer_hint(/*width*/ 80);
    assert!(notice.is_some());
    assert_ne!(notice, hint);
    let notice_line = notice.as_ref().map(|notice| notice.line.clone());
    app.transcript_view.begin_search();
    assert_eq!(app.composer_hint(/*width*/ 80), None);
    app.transcript_view = Default::default();
    assert_eq!(app.composer_hint(/*width*/ 80), notice);
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert_eq!(app.composer_hint(/*width*/ 80), None);
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    assert_eq!(app.composer_hint(/*width*/ 80), notice);

    for (spend_control, reached_type) in [
        (Some(true), None),
        (
            None,
            Some(codex_app_server_protocol::RateLimitReachedType::RateLimitReached),
        ),
    ] {
        let mut update = quota(/*used_percent*/ 92);
        update.spend_control_reached = spend_control;
        update.rate_limit_reached_type = reached_type;
        app.chat_widget.on_rate_limit_snapshot(Some(update));
        assert_eq!(app.chat_widget.usage_notice(/*width*/ 80), None);
    }
    app.chat_widget
        .on_rate_limit_snapshot(Some(quota(/*used_percent*/ 92)));
    assert_eq!(app.chat_widget.usage_notice(/*width*/ 78), notice_line);
    app.chat_widget.update_backend_banner(
        &serde_json::from_value(serde_json::json!({
            "rateLimits": quota(/*used_percent*/ 92),
            "rateLimitUpsell": {
                "banner_type": "selected_model_limit", "title": "Usage limit reached",
                "description": "Choose another model.", "ctas": [],
            },
        }))
        .unwrap(),
    );
    assert_eq!(app.chat_widget.usage_notice(/*width*/ 80), None);
    app.chat_widget.clear_backend_banner();
    assert_eq!(app.chat_widget.usage_notice(/*width*/ 78), notice_line);
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
