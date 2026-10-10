//! Config-panel persistence and delayed notification-preview behavior.

use super::*;
use crate::AppServerTarget;
use crate::app::test_support::make_test_app;
use crate::app_event::AppEvent;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::local_settings::LocalSettings;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn config_toggle_save_uses_host_config_while_offline() -> Result<()> {
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let local_cwd = root.path().join("local");
    let session_cwd = root.path().join("session");
    std::fs::create_dir_all(&home)?;
    for (cwd, auto_recap) in [(&local_cwd, false), (&session_cwd, true)] {
        std::fs::create_dir_all(cwd.join(".codex"))?;
        std::fs::write(
            cwd.join(".codex/config.toml"),
            format!("[tui]\nauto_recap = {auto_recap}\n"),
        )?;
    }
    let local_cwd = AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(local_cwd)?)?;
    let session_cwd = AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(session_cwd)?)?;
    ConfigEditsBuilder::new(&home)
        .set_project_trust_level(local_cwd.to_path_buf(), TrustLevel::Trusted)
        .set_project_trust_level(session_cwd.to_path_buf(), TrustLevel::Trusted)
        .apply()
        .await
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .codex_home(home)
        .harness_overrides(ConfigOverrides {
            cwd: Some(local_cwd.to_path_buf()),
            ..Default::default()
        })
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;
    assert!(!config.tui_auto_recap);

    let mut app = Box::pin(make_test_app()).await;
    app.config = config.clone();
    app.local_settings = LocalSettings::from(&config);
    app.launch_cwd = local_cwd.to_path_buf();
    app.config.cwd = session_cwd.clone();
    app.loader_overrides = loader_overrides;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: "ws://127.0.0.1:8765".to_string(),
            auth_token: None,
        },
    };
    app.reconnect.offline = true;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut session_config = config;
    session_config.cwd = session_cwd.clone();
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        session_config,
        /*initial_user_message*/ None,
    );
    app.chat_widget = ChatWidget::new_with_app_event(init);
    app.local_settings.tui.auto_recap = true;
    app.chat_widget.local_settings.tui.auto_recap = true;

    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
            setting: ConfigToggle::AutomaticRecaps,
            enabled: false,
        }),
    ))
    .await?;

    assert_eq!(
        (
            app.local_settings.tui.auto_recap,
            app.chat_widget.local_settings.tui.auto_recap,
            app.chat_widget.config_panel_tui.auto_recap,
        ),
        (true, true, false),
    );
    let saved: toml::Value = toml::from_str(&std::fs::read_to_string(
        &app.local_settings.user_config_path,
    )?)?;
    assert_eq!(saved["tui"]["auto_recap"].as_bool(), Some(false));
    assert_eq!(app.launch_cwd, local_cwd.to_path_buf());
    assert_eq!(
        (&app.config.cwd, &app.chat_widget.config_ref().cwd),
        (&session_cwd, &session_cwd),
    );

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn notification_test_delay_rejects_cancelled_and_rescheduled_timers() -> Result<()> {
    let (mut app, mut rx, _ops) = crate::app::tests::make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let notification_generation = |event| match event {
        AppEvent::SendConfigTestNotification { generation } => generation,
        _ => panic!("expected notification preview timer"),
    };
    tui.set_notification_settings(
        codex_config::types::NotificationMethod::Bel,
        codex_config::types::NotificationCondition::Unfocused,
    );
    app.chat_widget.open_config_panel();
    app.chat_widget.handle_key_event(KeyCode::Right.into());
    app.chat_widget.handle_key_event(KeyCode::Char('/').into());
    app.chat_widget.handle_paste("test notification".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().expect("test notification action"),
        AppEvent::TestConfigNotification
    ));
    app.schedule_config_notification_test();
    app.schedule_config_notification_test();
    tokio::task::yield_now().await;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(rx.try_recv().is_err());

    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 4)).await;
    assert!(rx.try_recv().is_err());
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 1)).await;
    tokio::task::yield_now().await;
    let generation = notification_generation(rx.try_recv().expect("scheduled preview"));
    assert!(rx.try_recv().is_err());

    app.send_config_notification_test(&mut tui, generation);
    let AppEvent::InsertHistoryCell(cell) = rx.try_recv()? else {
        panic!("expected notification result in history");
    };
    assert!(
        cell.display_lines(/*width*/ 120)
            .iter()
            .any(|line| line.to_string().starts_with("• Test sent to terminal · "))
    );

    app.schedule_config_notification_test();
    let cancelled_generation = app
        .config_notification_test_pending
        .expect("pending preview");
    tokio::task::yield_now().await;
    let (replacement, _, mut replacement_events, _) = make_chatwidget_manual_with_sender().await;
    app.replace_chat_widget(replacement);
    assert!(app.config_notification_test_pending.is_none());
    app.schedule_config_notification_test();
    let rescheduled_generation = app
        .config_notification_test_pending
        .expect("rescheduled preview");
    assert_ne!(cancelled_generation, rescheduled_generation);
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 5)).await;
    tokio::task::yield_now().await;
    let mut timer_generations = Vec::new();
    for _ in 0..2 {
        timer_generations.push(notification_generation(
            rx.try_recv().expect("preview timer"),
        ));
    }
    timer_generations.sort_unstable();
    assert_eq!(
        timer_generations,
        vec![cancelled_generation, rescheduled_generation]
    );

    app.send_config_notification_test(&mut tui, cancelled_generation);
    assert_eq!(
        app.config_notification_test_pending,
        Some(rescheduled_generation)
    );
    assert!(rx.try_recv().is_err());
    assert!(replacement_events.try_recv().is_err());
    app.send_config_notification_test(&mut tui, rescheduled_generation);
    assert!(matches!(
        replacement_events.try_recv(),
        Ok(AppEvent::InsertHistoryCell(_))
    ));

    app.schedule_config_notification_test();
    app.save_config_preference(
        &mut tui,
        ConfigPreference::Toggle {
            setting: ConfigToggle::Notifications,
            enabled: false,
        },
    )
    .await;
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 5)).await;
    tokio::task::yield_now().await;
    let generation = notification_generation(rx.try_recv().expect("cancelled preview timer"));
    app.send_config_notification_test(&mut tui, generation);
    assert!(rx.try_recv().is_err());
    Ok(())
}
