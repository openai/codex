use super::*;
use crate::app::owned_transcript::tests::buffer_text;
use crate::chatwidget::tests::helpers::complete_assistant_message;
use crate::chatwidget::tests::helpers::handle_agent_message_delta;
use futures::FutureExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn stream_animation_preserves_pacing_and_discards_stopped_ticks() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (replacement, _tx, _rx, _ops) = make_chatwidget_manual_with_sender().await;
    time::pause();
    let started_at = time::Instant::now();

    app.handle_event(&mut tui, &mut app_server, AppEvent::StartCommitAnimation)
        .await?;
    time::advance(COMMIT_ANIMATION_TICK / 2).await;
    app.handle_event(&mut tui, &mut app_server, AppEvent::StartCommitAnimation)
        .await?;
    time::advance(COMMIT_ANIMATION_TICK).await;
    let interval = app.commit_animation.as_mut().expect("animation started");
    assert_eq!(
        interval.tick().now_or_never(),
        Some(started_at + COMMIT_ANIMATION_TICK)
    );

    // A blocked UI consumes one overdue tick, not a burst of obsolete ticks.
    time::advance(COMMIT_ANIMATION_TICK * 4).await;
    assert!(interval.tick().now_or_never().is_some());
    assert!(interval.tick().now_or_never().is_none());
    let next_tick = time::Instant::now() + COMMIT_ANIMATION_TICK;
    time::advance(COMMIT_ANIMATION_TICK + Duration::from_millis(/*millis*/ 1)).await;
    assert_eq!(interval.tick().now_or_never(), Some(next_tick));

    // Restart while the previous timer is overdue; only the new deadline survives.
    time::advance(COMMIT_ANIMATION_TICK * 3).await;
    app.handle_event(&mut tui, &mut app_server, AppEvent::StopCommitAnimation)
        .await?;
    assert!(app.commit_animation.is_none());
    app.handle_event(&mut tui, &mut app_server, AppEvent::StartCommitAnimation)
        .await?;
    let interval = app.commit_animation.as_mut().expect("animation restarted");
    assert!(interval.tick().now_or_never().is_none());
    let next_tick = time::Instant::now() + COMMIT_ANIMATION_TICK;
    time::advance(COMMIT_ANIMATION_TICK + Duration::from_millis(/*millis*/ 1)).await;
    assert_eq!(interval.tick().now_or_never(), Some(next_tick));

    app.replace_chat_widget(replacement);
    assert!(app.commit_animation.is_none());
    time::resume();
    Ok(())
}

#[tokio::test]
async fn inline_streamed_table_stays_visible_until_history_is_painted() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let resized = tui.terminal.last_known_screen_size;
    let size = Size::new(/*width*/ 60, resized.height);
    let table = "| Pattern | Outcome |\n| --- | --- |\n| Table tail | Preserved |\n";
    handle_agent_message_delta(&mut app.chat_widget, table);
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
    let live = side_frame(&mut app, &mut tui, size)?;
    assert!(live.contains("Preserved"), "{live}");

    complete_assistant_message(&mut app.chat_widget, "msg-1", table, /*phase*/ None);
    // Postpone the paint without holding typeahead for a later approval prompt.
    assert!(app.can_read_terminal_after_thread_switch(&tui, &events));
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)
        .await?;
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyCode::Char('é').into()),
    )
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "é");
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(buffer_text(tui.terminal.previous_buffer()), live);
    // Widen the terminal while the table replay and the next message's final
    // insert are both pending. The prose should fit on one line at the new width.
    let prose = "This final sentence fits on one line only after the terminal widens.";
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(ThreadId::new(), "turn-1", "msg-2", prose),
        /*replay_kind*/ None,
    );
    complete_assistant_message(&mut app.chat_widget, "msg-2", prose, /*phase*/ None);
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Resize(resized))
        .await?;
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
    let resized = tui.screen_size_for_event(&TuiEvent::Draw)?;
    app.handle_draw_pre_render(&mut tui, resized)?;
    let history = tui.pending_history_lines_for_test();
    assert_snapshot!(history.iter().map(rendered_line_text).collect::<Vec<_>>().join("\n"), @r"
    •  Pattern       Outcome
      ━━━━━━━━━━━━  ━━━━━━━━━━━
       Table tail    Preserved

    • This final sentence fits on one line only after the terminal widens.
    ");
    app.render_chat_widget_frame(&mut tui, resized)?;
    let frame = buffer_text(tui.terminal.previous_buffer());
    assert!(
        !frame.contains("Preserved"),
        "table should appear only in scrollback"
    );
    server.shutdown().await?;
    Ok(())
}
