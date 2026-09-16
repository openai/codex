use super::*;
use crate::app::test_support::make_test_app;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::status::remote_connection::RemoteConnectionStatus;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn daemon_menu_is_read_only_and_confirmation_can_cancel_or_handoff() {
    let mut app = make_test_app().await;
    let (chat, _, mut rx, _) = make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat;
    let package = tempfile::tempdir().unwrap();
    std::fs::create_dir(package.path().join("bin")).unwrap();
    std::fs::write(package.path().join("bin/codex"), "CLI").unwrap();
    std::fs::write(package.path().join("codex-package.json"), "{}").unwrap();
    app.daemon_cli_executable =
        Some(AbsolutePathBuf::from_absolute_path(package.path().join("bin/codex")).unwrap());
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock").unwrap(),
        },
    };
    app.chat_widget.remote_connection = Some(RemoteConnectionStatus {
        address: "local".into(),
        version: "v0.153.0".into(),
    });
    app.initialize_server_version_notice("0.154.0", Some("0.153.0"));
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let overview = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    insta::assert_snapshot!(overview.lines().next().unwrap().trim(), @"Service v0.153.0 < Codex CLI v0.154.0 · /daemon");
    let executable = app.daemon_cli_executable.clone();
    for (width, source, snapshot) in [
        (
            80,
            DaemonUpdateSource::PublicStable,
            "daemon_stable_confirmation",
        ),
        (100, DaemonUpdateSource::ThisCli, "daemon_cli_confirmation"),
    ] {
        app.daemon_cli_executable = executable.clone();
        app.open_daemon_menu();
        if source == DaemonUpdateSource::PublicStable {
            insta::assert_snapshot!(
                "daemon_menu",
                render_bottom_popup(&app.chat_widget, /*width*/ 80)
            );
        } else {
            app.chat_widget.handle_key_event(KeyCode::Down.into());
        }
        assert!(rx.try_recv().is_err());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(
            matches!(rx.try_recv().unwrap(), AppEvent::ConfirmDaemonUpdate(selected) if selected == source)
        );
        app.daemon_cli_executable = Some(
            AbsolutePathBuf::from_absolute_path(if cfg!(windows) {
                r"C:\cli-build\bin\codex"
            } else {
                "/x/cli-build/bin/codex"
            })
            .unwrap(),
        );
        app.confirm_daemon_update(source);
        insta::assert_snapshot!(
            snapshot,
            render_bottom_popup(&app.chat_widget, width)
                .replace(r"C:\cli-build\bin\codex", "/x/cli-build/bin/codex")
        );
        // The default choice cancels without emitting an update or exiting.
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(rx.try_recv().is_err());
        assert_eq!(app.pending_update_action, None);
        app.confirm_daemon_update(source);
        app.chat_widget.handle_key_event(KeyCode::Down.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(
            matches!(rx.try_recv().unwrap(), AppEvent::RunDaemonUpdate(selected) if selected == source)
        );
    }

    // Maintenance remains available when startup selects the embedded server.
    app.app_server_target = AppServerTarget::Embedded;
    app.chat_widget.remote_connection = None;
    app.daemon_cli_executable = executable;
    for source in [
        DaemonUpdateSource::PublicStable,
        DaemonUpdateSource::ThisCli,
    ] {
        app.open_daemon_menu();
        if source == DaemonUpdateSource::PublicStable {
            insta::assert_snapshot!(
                "daemon_disconnected",
                render_bottom_popup(&app.chat_widget, /*width*/ 80)
            );
        } else {
            app.chat_widget.handle_key_event(KeyCode::Down.into());
        }
        assert!(rx.try_recv().is_err());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(
            matches!(rx.try_recv().unwrap(), AppEvent::ConfirmDaemonUpdate(selected) if selected == source)
        );
        app.confirm_daemon_update(source);
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(rx.try_recv().is_err());
        assert_eq!(app.pending_update_action, None);
        app.confirm_daemon_update(source);
        app.chat_widget.handle_key_event(KeyCode::Down.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(
            matches!(rx.try_recv().unwrap(), AppEvent::RunDaemonUpdate(selected) if selected == source)
        );
    }

    std::fs::remove_file(package.path().join("codex-package.json")).unwrap();
    // The full CLI can have any filename; capability comes from its entry point.
    app.daemon_cli_executable =
        Some(AbsolutePathBuf::from_absolute_path(package.path().join("bin/codex-tui")).unwrap());
    app.open_daemon_menu();
    insta::assert_snapshot!(
        "daemon_unpackaged_cli",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(
        rx.try_recv().unwrap(),
        AppEvent::ConfirmDaemonUpdate(DaemonUpdateSource::PublicStable)
    ));

    app.daemon_cli_executable = None;
    app.open_daemon_menu();
    insta::assert_snapshot!(
        "daemon_no_cli_handoff",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn unavailable_daemon_menu_offers_guidance_without_update_actions() {
    let mut app = make_test_app().await;
    app.daemon_cli_executable =
        Some(AbsolutePathBuf::from_absolute_path(std::env::current_exe().unwrap()).unwrap());
    let (chat, _, mut rx, _) = make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: "ws://example.test:1234".into(),
            auth_token: None,
        },
    };
    app.chat_widget.remote_connection = Some(RemoteConnectionStatus {
        address: "ws://example.test:1234".into(),
        version: "v0.153.0".into(),
    });
    app.open_daemon_menu();
    insta::assert_snapshot!(
        "daemon_remote_guidance",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(rx.try_recv().is_err());
    assert_eq!(app.pending_update_action, None);
}
