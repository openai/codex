use super::*;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::CreditsSnapshot;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RateLimitWindow;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;

fn rate_limit_snapshot(
    used_percent: i32,
    rate_limit_reached_type: Option<RateLimitReachedType>,
    spend_control_reached: Option<bool>,
) -> RateLimitSnapshot {
    RateLimitSnapshot {
        limit_id: Some("codex".to_string()),
        limit_name: None,
        normal_model_slug: None,
        primary: Some(RateLimitWindow {
            used_percent,
            window_duration_mins: Some(300),
            resets_at: None,
        }),
        secondary: None,
        credits: Some(CreditsSnapshot {
            has_credits: true,
            unlimited: false,
            balance: None,
        }),
        individual_limit: None,
        spend_control_reached,
        plan_type: None,
        rate_limit_reached_type,
    }
}

fn account_rate_limits_response(snapshot: RateLimitSnapshot) -> GetAccountRateLimitsResponse {
    GetAccountRateLimitsResponse {
        ordinary_usage_allowed: None,
        account_id: None,
        rate_limit_upsell: None,
        rate_limits: snapshot,
        rate_limits_by_limit_id: None,
        rate_limit_reset_credits: Some(RateLimitResetCreditsSummary {
            available_count: 0,
            credits: None,
        }),
    }
}

async fn deliver_rolling_rate_limit_snapshot(
    app: &mut App,
    app_server: &AppServerSession,
    snapshot: RateLimitSnapshot,
) {
    app.handle_app_server_event(
        app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::AccountRateLimitsUpdated(AccountRateLimitsUpdatedNotification {
                rate_limits: snapshot,
            }),
        )),
    )
    .await;
}

fn render_status_output(
    app: &mut App,
    app_event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> String {
    while app_event_rx.try_recv().is_ok() {}
    app.chat_widget.add_status_output(
        /*refreshing_rate_limits*/ false, /*request_id*/ None,
    );
    match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell
            .display_lines(/*width*/ 120)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("expected status output, got {other:?}"),
    }
}

fn deliver_usage_limit_error(app: &mut App) {
    app.chat_widget.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                misalignment: None,
                message: "Usage limit reached.".to_string(),
                codex_error_info: Some(CodexErrorInfo::UsageLimitExceeded),
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );
}

#[tokio::test]
async fn backend_banner_state_survives_widget_replacement() -> Result<()> {
    for dismiss in [false, true] {
        let (mut app, _rx, _op_rx) = make_test_app_with_channels().await;
        set_chatgpt_auth(&mut app.chat_widget);
        let mut response = account_rate_limits_response(rate_limit_snapshot(
            /*used_percent*/ 100, /*rate_limit_reached_type*/ None,
            /*spend_control_reached*/ None,
        ));
        response.rate_limit_upsell = Some(serde_json::json!({
            "banner_type": "plus_rate_limit_reached", "title": "Usage limit reached",
            "presentation": "dismissible",
            "description": "Choose how to continue.",
            "ctas": [{"action": "view_usage", "label": "View usage"}]
        }));
        app.chat_widget.update_backend_banner(&response);
        assert!(render_bottom_popup(&app.chat_widget, /*width*/ 90).contains("View usage"));
        if dismiss {
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        }
        let before = render_bottom_popup(&app.chat_widget, /*width*/ 90);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        set_active_cell(
            &mut app.chat_widget,
            Box::new(PlainHistoryCell::new(Vec::new())),
        );
        app.chat_widget.pre_draw_tick();
        assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 90), before);
        app.chat_widget.update_backend_banner(&response);
        assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 90), before);
        response.rate_limit_upsell = None;
        app.chat_widget.update_backend_banner(&response);
        assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 90).contains("View usage"));
    }
    Ok(())
}

#[tokio::test]
async fn rolling_workspace_hard_stops_invalidate_older_rate_limit_reads() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");

    let cases = [
        (None, None, false),
        (Some(RateLimitReachedType::RateLimitReached), None, false),
        (None, Some(false), false),
        (None, Some(true), true),
        (
            Some(RateLimitReachedType::WorkspaceOwnerCreditsDepleted),
            None,
            true,
        ),
        (
            Some(RateLimitReachedType::WorkspaceMemberCreditsDepleted),
            None,
            true,
        ),
        (
            Some(RateLimitReachedType::WorkspaceOwnerUsageLimitReached),
            None,
            true,
        ),
        (
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
            None,
            true,
        ),
    ];
    let mut expected_generation = 0;
    for (reached_type, spend_control_reached, invalidates) in cases {
        deliver_rolling_rate_limit_snapshot(
            &mut app,
            &app_server,
            rate_limit_snapshot(
                /*used_percent*/ 95,
                reached_type,
                spend_control_reached,
            ),
        )
        .await;
        if invalidates {
            expected_generation += 1;
        }
        assert_eq!(
            app.rate_limit_hard_stop_generation, expected_generation,
            "reached_type={reached_type:?}, spend_control_reached={spend_control_reached:?}"
        );
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stale_rate_limit_reads_preserve_newer_workspace_hard_stop_for_every_origin() -> Result<()>
{
    for origin_name in [
        "startup",
        "status",
        "usage",
        "reset-picker",
        "reset-consume",
    ] {
        let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
        set_chatgpt_auth(&mut app.chat_widget);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
            app.chat_widget.config_ref(),
        ))
        .await?;

        app.chat_widget
            .on_rate_limit_snapshot(Some(rate_limit_snapshot(
                /*used_percent*/ 20,
                /*rate_limit_reached_type*/ None,
                Some(false),
            )));
        let origin = match origin_name {
            "startup" => RateLimitRefreshOrigin::StartupPrefetch {
                reset_hint_request_id: app.chat_widget.start_rate_limit_reset_startup_check(),
            },
            "status" => {
                let request_id = 7;
                app.chat_widget
                    .add_status_output(/*refreshing_rate_limits*/ true, Some(request_id));
                RateLimitRefreshOrigin::StatusCommand { request_id }
            }
            "usage" => {
                let startup_request_id = app.chat_widget.start_rate_limit_reset_startup_check();
                app.chat_widget.finish_rate_limit_reset_hint_refresh(
                    startup_request_id,
                    Vec::new(),
                    Ok(RateLimitResetCreditsSummary {
                        available_count: 0,
                        credits: None,
                    }),
                );
                app.chat_widget.insert_str("/usage");
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                loop {
                    match app_event_rx.try_recv() {
                        Ok(AppEvent::RefreshRateLimits { origin }) => break origin,
                        Ok(_) => {}
                        other => panic!("expected usage refresh request, got {other:?}"),
                    }
                }
            }
            "reset-picker" => RateLimitRefreshOrigin::ResetPicker {
                request_id: app.chat_widget.show_rate_limit_reset_loading_popup(),
            },
            "reset-consume" => RateLimitRefreshOrigin::ResetConsume {
                request_id: app.chat_widget.show_rate_limit_reset_consuming_popup(),
            },
            _ => unreachable!("unknown refresh origin"),
        };
        let read_generation = app.rate_limit_hard_stop_generation;
        let mut rolling_snapshot = rate_limit_snapshot(
            /*used_percent*/ 95,
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
            Some(true),
        );
        if origin_name == "reset-picker" {
            rolling_snapshot.limit_id = Some("codex_other".to_string());
        }
        deliver_rolling_rate_limit_snapshot(&mut app, &app_server, rolling_snapshot).await;
        assert_ne!(read_generation, app.rate_limit_hard_stop_generation);

        let control = Box::pin(app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::RateLimitsLoaded {
                request_id: 0,
                origin,
                hard_stop_generation: read_generation,
                result: Ok(account_rate_limits_response(rate_limit_snapshot(
                    /*used_percent*/ 0,
                    /*rate_limit_reached_type*/ None,
                    Some(false),
                ))),
            },
        ))
        .await?;
        assert!(matches!(control, AppRunControl::Continue));

        let popup = render_bottom_popup(&app.chat_widget, /*width*/ 100);
        match origin_name {
            "usage" => assert!(
                popup.contains("Redeem reset    None available."),
                "expected usage reset availability, got: {popup}"
            ),
            "reset-picker" => {
                assert!(popup.contains("You don't have any usage limit resets available."));
            }
            "reset-consume" => {
                assert!(popup.contains("Usage reset. You have 0 usage limit resets left."));
            }
            "startup" | "status" => {}
            _ => unreachable!("unknown refresh origin"),
        }

        let status = render_status_output(&mut app, &mut app_event_rx);
        assert!(
            status.contains("80% left"),
            "expected {origin_name} to preserve the last account usage snapshot, got: {status}"
        );
        deliver_usage_limit_error(&mut app);
        let popup = render_bottom_popup(&app.chat_widget, /*width*/ 100);
        assert!(
            popup.contains("Request a limit increase from your owner"),
            "expected {origin_name} to preserve workspace error routing, got: {popup}"
        );

        app_server.shutdown().await?;
    }

    Ok(())
}

#[tokio::test]
async fn stale_rate_limit_read_does_not_dismiss_visible_workspace_advisory() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    set_chatgpt_auth(&mut app.chat_widget);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    let request_id = 7;
    app.chat_widget
        .add_status_output(/*refreshing_rate_limits*/ true, Some(request_id));
    let read_generation = app.rate_limit_hard_stop_generation;

    deliver_rolling_rate_limit_snapshot(
        &mut app,
        &app_server,
        rate_limit_snapshot(
            /*used_percent*/ 95,
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
            Some(true),
        ),
    )
    .await;
    app.chat_widget.handle_server_notification(
        turn_completed_notification(ThreadId::new(), "turn-1", TurnStatus::Completed),
        /*replay_kind*/ None,
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("Approaching rate limits")
    );

    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::RateLimitsLoaded {
            request_id: 0,
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
            hard_stop_generation: read_generation,
            result: Ok(account_rate_limits_response(rate_limit_snapshot(
                /*used_percent*/ 0,
                /*rate_limit_reached_type*/ None,
                Some(false),
            ))),
        },
    ))
    .await?;

    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("Approaching rate limits")
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn post_hard_stop_rate_limit_read_clears_recovered_workspace_limit() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    set_chatgpt_auth(&mut app.chat_widget);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    deliver_rolling_rate_limit_snapshot(
        &mut app,
        &app_server,
        rate_limit_snapshot(
            /*used_percent*/ 95,
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
            Some(true),
        ),
    )
    .await;
    let read_generation = app.rate_limit_hard_stop_generation;
    let request_id = 7;
    app.chat_widget
        .add_status_output(/*refreshing_rate_limits*/ true, Some(request_id));

    let control = Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::RateLimitsLoaded {
            request_id: 0,
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
            hard_stop_generation: read_generation,
            result: Ok(account_rate_limits_response(rate_limit_snapshot(
                /*used_percent*/ 0,
                /*rate_limit_reached_type*/ None,
                Some(false),
            ))),
        },
    ))
    .await?;
    assert!(matches!(control, AppRunControl::Continue));

    let status = render_status_output(&mut app, &mut app_event_rx);
    assert!(
        status.contains("100% left"),
        "expected recovered limits, got: {status}"
    );
    deliver_usage_limit_error(&mut app);
    let popup = render_bottom_popup(&app.chat_widget, /*width*/ 100);
    assert!(
        !popup.contains("Request a limit increase from your owner"),
        "expected recovered state to clear workspace error routing, got: {popup}"
    );

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failed_rate_limit_read_preserves_visible_backend_banner() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await?;
    let mut response = account_rate_limits_response(rate_limit_snapshot(
        /*used_percent*/ 25,
        /*rate_limit_reached_type*/ None,
        Some(false),
    ));
    response.account_id = Some("workspace-a".into());
    response.rate_limit_upsell = Some(serde_json::json!({
        "banner_type": "workspace_recovery", "presentation": "inline",
        "title": "Workspace needs credits", "description": "Ask your owner for credits.",
        "ctas": []
    }));
    app.chat_widget.update_backend_banner(&response);
    let before = render_bottom_popup(&app.chat_widget, /*width*/ 90);
    assert!(before.contains("Workspace needs credits"));
    let request_id = 7;
    app.chat_widget
        .add_status_output(/*refreshing_rate_limits*/ true, Some(request_id));
    let generation = app.rate_limit_hard_stop_generation;
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::RateLimitsLoaded {
            request_id: 0,
            origin: RateLimitRefreshOrigin::StatusCommand { request_id },
            hard_stop_generation: generation,
            result: Err("transient test failure".into()),
        },
    )
    .await?;
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 90), before);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn backend_banner_reads_ignore_older_completions() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let mut session = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.set_model("test-model-a");
    let mut current = account_rate_limits_response(rate_limit_snapshot(
        /*used_percent*/ 100, /*rate_limit_reached_type*/ None,
        /*spend_control_reached*/ None,
    ));
    current.rate_limit_upsell = Some(serde_json::json!({
        "banner_type":"selected_model_limit", "model_slug":"test-model-a", "presentation":"inline",
        "title":"Selected model usage exhausted", "description":"Contact your owner.", "ctas":[]
    }));
    for request_id in 1..=5 {
        app.rate_limit_refresh_state
            .start(
                RateLimitRefreshOrigin::StatusCommand { request_id },
                &mut app.rate_limit_hard_stop_generation,
            )
            .unwrap();
    }
    for (id, generation, result, expect_visible) in [
        (2, 0, Ok(current.clone()), true),
        (
            1,
            0,
            Ok({
                let mut absent = current.clone();
                absent.rate_limit_upsell = None;
                absent
            }),
            true,
        ),
        (4, 0, Err("transient failure".into()), true),
        (
            3,
            0,
            Ok({
                let mut absent = current.clone();
                absent.rate_limit_upsell = None;
                absent
            }),
            false,
        ),
        (5, 0, Ok(current.clone()), true),
    ] {
        app.handle_event(
            &mut tui,
            &mut session,
            AppEvent::RateLimitsLoaded {
                request_id: id,
                origin: RateLimitRefreshOrigin::StatusCommand { request_id: id },
                hard_stop_generation: generation,
                result,
            },
        )
        .await?;
        assert_eq!(
            render_bottom_popup(&app.chat_widget, /*width*/ 90)
                .contains("Selected model usage exhausted"),
            expect_visible
        );
    }
    session.shutdown().await?;
    Ok(())
}
