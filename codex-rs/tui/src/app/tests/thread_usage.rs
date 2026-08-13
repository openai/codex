use super::*;
use crate::chatwidget::ThreadUsageOutcome;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ThreadUsage;
use codex_protocol::account::PlanType;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::layout::Size;

async fn app_with_pending_thread_usage() -> Result<(App, AppServerSession, tui::Tui, ThreadId, u64)>
{
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.chat_widget.handle_thread_session(test_thread_session(
        thread_id,
        test_path_buf("/tmp/project"),
    ));
    app.chat_widget.update_account_state(
        /*status_account_display*/ None,
        Some(PlanType::Business),
        /*has_chatgpt_account*/ false,
        /*has_codex_backend_auth*/ true,
    );
    while app_event_rx.try_recv().is_ok() {}
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 21, /*width*/ 80, /*height*/ 3,
    ));

    app.chat_widget.add_status_output(
        /*refreshing_rate_limits*/ false, /*request_id*/ None,
    );
    let initial_status = match app_event_rx.try_recv() {
        Ok(event @ AppEvent::InsertHistoryCell(_)) => event,
        other => panic!("expected immediate status card, got {other:?}"),
    };
    let request_id = match app_event_rx.try_recv() {
        Ok(AppEvent::RefreshThreadUsage {
            thread_id: requested_thread_id,
            request_id,
        }) if requested_thread_id == thread_id => request_id,
        other => panic!("expected asynchronous thread usage request, got {other:?}"),
    };
    app.handle_event(&mut tui, &mut app_server, initial_status)
        .await?;

    Ok((app, app_server, tui, thread_id, request_id))
}

fn successful_thread_usage(thread_id: ThreadId, request_id: u64) -> AppEvent {
    AppEvent::ThreadUsageLoaded {
        thread_id,
        request_id,
        result: Ok(ThreadUsageOutcome::Available(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 50_000_000,
            estimated_usage_usd_micros: Some(1_820_000),
            groups: Vec::new(),
        })),
    }
}

fn pending_history_text(tui: &tui::Tui) -> String {
    tui.pending_history_lines_for_test()
        .into_iter()
        .map(|line| {
            line.line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn account_updated_with_backend_only_auth_enables_thread_usage() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.chat_widget.handle_thread_session(test_thread_session(
        thread_id,
        test_path_buf("/tmp/project"),
    ));
    while app_event_rx.try_recv().is_ok() {}
    let app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountUpdated(
            AccountUpdatedNotification {
                auth_mode: Some(AuthMode::AgentIdentity),
                plan_type: Some(PlanType::Business),
            },
        ))),
    )
    .await;

    assert!(!app.chat_widget.has_chatgpt_account());
    assert!(app.chat_widget.has_codex_backend_auth());
    app.chat_widget.add_status_output(
        /*refreshing_rate_limits*/ false, /*request_id*/ None,
    );
    assert_matches!(app_event_rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::RefreshThreadUsage {
            thread_id: requested_thread_id,
            ..
        }) if requested_thread_id == thread_id
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn completed_thread_usage_updates_status_without_scrollback_reflow() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    assert!(!app.transcript_reflow.has_pending_reflow());
    assert!(pending_history_text(&tui).contains("/status"));

    app.handle_event(
        &mut tui,
        &mut app_server,
        successful_thread_usage(thread_id, request_id),
    )
    .await?;

    assert!(!app.transcript_reflow.has_pending_reflow());
    assert!(
        tui.pending_history_lines_for_test().is_empty(),
        "a visible status tail should be replaced directly instead of appending a duplicate card"
    );
    let status = app
        .transcript_cells
        .last()
        .expect("status card should remain in terminal history");
    let rendered = lines_to_single_string(&status.display_lines(/*width*/ 90));
    assert!(rendered.contains("50 credits"), "{rendered}");
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn thread_usage_after_intervening_history_appends_refreshed_status() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    app.insert_history_cell(
        &mut tui,
        Box::new(PlainHistoryCell::new(vec![
            Line::from("/status"),
            Line::from("intervening model output"),
        ])),
    );

    app.handle_event(
        &mut tui,
        &mut app_server,
        successful_thread_usage(thread_id, request_id),
    )
    .await?;

    let pending = pending_history_text(&tui);
    assert!(pending.contains("intervening model output"), "{pending}");
    assert!(pending.contains("50 credits"), "{pending}");
    assert!(!app.transcript_reflow.has_pending_reflow());
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failed_thread_usage_after_intervening_history_does_not_duplicate_status() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    app.insert_history_cell(
        &mut tui,
        Box::new(PlainHistoryCell::new(vec![Line::from(
            "intervening model output",
        )])),
    );
    let pending_before_failure = pending_history_text(&tui);

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::ThreadUsageLoaded {
            thread_id,
            request_id,
            result: Err("thread usage request timed out".to_string()),
        },
    )
    .await?;

    assert_eq!(pending_history_text(&tui), pending_before_failure);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn thread_usage_finishing_during_overlay_updates_history_after_close() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    tui.set_alt_screen_enabled(/*enabled*/ false);
    app.open_transcript_overlay(&mut tui);

    app.handle_event(
        &mut tui,
        &mut app_server,
        successful_thread_usage(thread_id, request_id),
    )
    .await?;
    assert!(app.pending_thread_usage_history_refresh);

    app.close_transcript_overlay(&mut tui);

    assert!(!app.pending_thread_usage_history_refresh);
    assert!(tui.pending_history_lines_for_test().is_empty());
    assert!(
        app.last_rendered_history_tail
            .as_ref()
            .expect("updated status should remain the rendered history tail")
            .lines
            .iter()
            .any(|line| line.line.to_string().contains("50 credits"))
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn account_change_discards_thread_usage_deferred_while_overlay_is_open() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    tui.set_alt_screen_enabled(/*enabled*/ false);
    app.open_transcript_overlay(&mut tui);
    app.handle_event(
        &mut tui,
        &mut app_server,
        successful_thread_usage(thread_id, request_id),
    )
    .await?;
    assert!(app.pending_thread_usage_history_refresh);

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountUpdated(
            AccountUpdatedNotification {
                auth_mode: Some(AuthMode::AgentIdentity),
                plan_type: Some(PlanType::Business),
            },
        ))),
    )
    .await;
    assert!(!app.pending_thread_usage_history_refresh);
    assert!(app.last_thread_usage_status_cell.is_none());

    app.close_transcript_overlay(&mut tui);

    assert!(!pending_history_text(&tui).contains("50 credits"));
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn terminal_reflow_rebases_pending_status_update_to_new_width() -> Result<()> {
    let (mut app, mut app_server, mut tui, thread_id, request_id) =
        app_with_pending_thread_usage().await?;
    let original = app
        .last_rendered_history_tail
        .as_ref()
        .expect("initial rendered status")
        .lines
        .clone();
    let screen_size = Size::new(/*width*/ 52, /*height*/ 24);
    tui.terminal.last_known_screen_size = screen_size;
    let viewport = Rect::new(
        /*x*/ 0,
        /*y*/ 21,
        screen_size.width,
        /*height*/ 3,
    );
    tui.terminal.set_viewport_area(viewport);

    app.reflow_transcript_now(&mut tui, screen_size.into())?;

    let resized = app
        .last_rendered_history_tail
        .as_ref()
        .expect("resized rendered status")
        .lines
        .clone();
    assert_ne!(original, resized);
    // Bazel tests run without a TTY, so model the settled viewport explicitly
    // before simulating an asynchronous usage update after resize replay.
    tui.terminal.set_viewport_area(viewport);
    app.handle_event(
        &mut tui,
        &mut app_server,
        successful_thread_usage(thread_id, request_id),
    )
    .await?;
    assert!(tui.pending_history_lines_for_test().is_empty());
    assert!(!app.transcript_reflow.has_pending_reflow());
    app_server.shutdown().await?;
    Ok(())
}
