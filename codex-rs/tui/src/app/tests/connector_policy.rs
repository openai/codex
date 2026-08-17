use super::*;
use crate::app_event::ConnectorsSnapshot;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AppListUpdatedNotification;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn installed_connector_mentions_ignore_stale_thread_workspace_and_account() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.chat_widget
        .set_feature_enabled(Feature::Apps, /*enabled*/ true);
    let thread_id = ThreadId::new();
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, cwd.clone()));
    let previous_generation = app.chat_widget.connector_scope_generation();
    app.chat_widget.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    app.chat_widget.insert_str("$");

    let generation = app.chat_widget.connector_scope_generation();
    let snapshot = ConnectorsSnapshot {
        connectors: vec![serde_json::from_str(
            r#"{"id":"scoped-app","name":"Scoped App","isAccessible":true}"#,
        )?],
    };
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    for (candidate_thread, candidate_cwd, candidate_generation, is_current) in [
        (Some(ThreadId::new()), cwd.clone(), generation, false),
        (
            Some(thread_id),
            cwd.join("other-workspace"),
            generation,
            false,
        ),
        (Some(thread_id), cwd.clone(), previous_generation, false),
        (Some(thread_id), cwd.clone(), generation, true),
    ] {
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::InstalledConnectorMentionsLoaded {
                thread_id: candidate_thread,
                cwd: candidate_cwd,
                generation: candidate_generation,
                result: Ok(snapshot.clone()),
            },
        )
        .await?;
        assert_eq!(
            render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Scoped App"),
            is_current
        );
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn queued_connector_fetches_are_bound_to_their_original_account() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    app.chat_widget
        .handle_thread_session(test_thread_session(ThreadId::new(), cwd));
    while app_event_rx.try_recv().is_ok() {}

    app.chat_widget.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    let stale_fetch = app_event_rx.try_recv()?;
    app.chat_widget.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    let current_fetch = app_event_rx.try_recv()?;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let metrics = tokio::runtime::Handle::current().metrics();
    let initial_tasks = metrics.num_alive_tasks();

    app.handle_event(&mut tui, &mut app_server, stale_fetch)
        .await?;
    assert_eq!(metrics.num_alive_tasks(), initial_tasks);
    app.handle_event(&mut tui, &mut app_server, current_fetch)
        .await?;
    assert_eq!(metrics.num_alive_tasks(), initial_tasks + 1);

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn app_list_notifications_revalidate_installed_mentions_and_the_current_directory()
-> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, cwd.clone()));
    app.chat_widget.update_account_state(
        /*status_account_display*/ None, /*plan_type*/ None,
        /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
    );
    let scoped_snapshot = ConnectorsSnapshot {
        connectors: vec![serde_json::from_str(
            r#"{"id":"scoped-app","name":"Scoped App","isAccessible":true}"#,
        )?],
    };
    app.chat_widget
        .on_connectors_loaded(Ok(scoped_snapshot.clone()), /*is_final*/ true);
    app.chat_widget.on_connector_mentions_loaded(
        app.chat_widget.connector_scope_generation(),
        Ok(scoped_snapshot.clone()),
    );
    app.chat_widget.insert_str("$");
    while app_event_rx.try_recv().is_ok() {}

    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    for (id, name, should_refresh) in [
        ("scoped-app", "Scoped App", false),
        ("old-workspace", "Old Workspace", true),
        ("old-workspace", "Old Workspace", false),
    ] {
        let notification = ServerNotification::AppListUpdated(AppListUpdatedNotification {
            data: vec![serde_json::from_value(serde_json::json!({
                "id": id, "name": name, "isAccessible": true
            }))?],
        });
        app.handle_app_server_event(
            &app_server,
            AppServerEvent::ServerNotification(Box::new(notification)),
        )
        .await;

        if should_refresh {
            assert!(matches!(
                app_event_rx.try_recv(),
                Ok(AppEvent::FetchInstalledConnectorMentions {
                    force_refresh: true,
                    ..
                })
            ));
        }
        assert_eq!(
            matches!(
                app_event_rx.try_recv(),
                Ok(AppEvent::FetchConnectorsList {
                    force_refetch: false,
                    ..
                })
            ),
            should_refresh
        );
        let popup = render_bottom_popup(&app.chat_widget, /*width*/ 80);
        assert!(popup.contains("Scoped App"));
        assert!(!popup.contains("Old Workspace"));
    }

    let generation = app.chat_widget.connector_scope_generation();
    app.chat_widget.on_connector_mentions_loaded(
        generation,
        Ok(ConnectorsSnapshot {
            connectors: vec![serde_json::from_str(
                r#"{"id":"old-workspace","name":"Old Workspace","isAccessible":true}"#,
            )?],
        }),
    );
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Old Workspace"));
    assert!(app_event_rx.try_recv().is_err());

    // Revocations refresh mentions even if discovery already has this notification.
    let revoked_notification = ServerNotification::AppListUpdated(AppListUpdatedNotification {
        data: vec![serde_json::from_str(
            r#"{"id":"scoped-app","name":"Scoped App","isAccessible":true}"#,
        )?],
    });
    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(revoked_notification)),
    )
    .await;
    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::FetchInstalledConnectorMentions {
            force_refresh: true,
            generation: requested_generation,
        }) if requested_generation == generation
    ));
    assert!(app_event_rx.try_recv().is_err());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Old Workspace"));
    app.chat_widget
        .on_connector_mentions_loaded(generation, Ok(scoped_snapshot.clone()));
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Old Workspace"));

    let ready_app = r#"{"id":"newly-ready","name":"Newly Ready","isAccessible":true}"#;
    let ready_notification = ServerNotification::AppListUpdated(AppListUpdatedNotification {
        data: vec![serde_json::from_str(ready_app)?],
    });
    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ready_notification.clone())),
    )
    .await;
    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::FetchInstalledConnectorMentions {
            force_refresh: true,
            ..
        })
    ));
    assert!(app_event_rx.try_recv().is_err());
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Newly Ready"));

    let mut tui = crate::tui::test_support::make_test_tui()?;
    for should_retry in [true, false, true, false] {
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::ConnectorsLoaded {
                thread_id: Some(thread_id),
                cwd: cwd.clone(),
                generation,
                result: Err("app directory failed".to_string()),
                is_final: true,
            },
        )
        .await?;
        assert_eq!(
            matches!(
                app_event_rx.try_recv(),
                Ok(AppEvent::FetchConnectorsList {
                    force_refetch: false,
                    ..
                })
            ),
            should_retry
        );
        assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Newly Ready"));
        app.handle_app_server_event(
            &app_server,
            AppServerEvent::ServerNotification(Box::new(ready_notification.clone())),
        )
        .await;
        assert_eq!(app_event_rx.try_recv().is_ok(), !should_retry);
        if !should_retry {
            app.handle_app_server_event(
                &app_server,
                AppServerEvent::ServerNotification(Box::new(ready_notification.clone())),
            )
            .await;
            assert!(app_event_rx.try_recv().is_err());
        }
    }

    for (snapshot, should_revalidate) in [
        (scoped_snapshot, true),
        (
            ConnectorsSnapshot {
                connectors: vec![serde_json::from_str(ready_app)?],
            },
            false,
        ),
    ] {
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::ConnectorsLoaded {
                thread_id: Some(thread_id),
                cwd: cwd.clone(),
                generation,
                result: Ok(snapshot),
                is_final: true,
            },
        )
        .await?;
        assert_eq!(
            matches!(
                app_event_rx.try_recv(),
                Ok(AppEvent::FetchConnectorsList {
                    force_refetch: false,
                    ..
                })
            ),
            should_revalidate
        );
        if should_revalidate {
            app.handle_app_server_event(
                &app_server,
                AppServerEvent::ServerNotification(Box::new(ready_notification.clone())),
            )
            .await;
            assert!(app_event_rx.try_recv().is_err());
        }
    }
    app.chat_widget.add_connectors_output();
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Newly Ready"));

    app_server.shutdown().await?;
    Ok(())
}
