#![cfg(test)]

use super::*;
use crate::app_server_session::ThreadParamsMode;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudOperation;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn rc_requests_durable_changes_and_manual_pairing() {
    let (client, mut commands, _events) = CloudClient::channel(/*capacity*/ 3);
    let session = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        ThreadParamsMode::Cloud,
    );
    for action in [Action::Enable, Action::Disable, Action::AddDevice] {
        let request = run(session.request_view(), action);
        let check = async {
            let command = commands.recv().await.unwrap();
            let CloudOperation::Request(request) = command.operation else {
                panic!("expected request")
            };
            assert!(matches!(
                (action, *request),
                (
                    Action::Enable,
                    ClientRequest::RemoteControlEnable { params: None, .. }
                ) | (
                    Action::Disable,
                    ClientRequest::RemoteControlDisable { params: None, .. }
                ) | (
                    Action::AddDevice,
                    ClientRequest::RemoteControlPairingStart {
                        params: RemoteControlPairingStartParams { manual_code: true },
                        ..
                    }
                )
            ));
            drop(command.response);
        };
        tokio::join!(request, check);
    }
}

#[tokio::test]
async fn rc_menu_snapshot() {
    let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
    let mut views = Vec::new();
    for state in [
        None,
        Some((RemoteControlConnectionStatus::Disabled, Some(false))),
        Some((RemoteControlConnectionStatus::Connected, Some(true))),
        Some((RemoteControlConnectionStatus::Disabled, Some(true))),
    ] {
        chat.show_selection_view(menu(
            state.map(|(status, saved)| (crate::backend_id::BackendId::new(), status, saved)),
            "Remote control requires the local background server",
        ));
        views.push(render_bottom_popup(&chat, /*width*/ 80));
    }
    assert_eq!(
        format!("{:?}", Outcome::Pairing("ABCD-EFGH".into())),
        "Pairing([redacted])"
    );
    chat.show_selection_view(pairing("ABCD-EFGH".into()));
    views.push(render_bottom_popup(&chat, /*width*/ 80));
    chat.show_selection_view(menu(/*connection*/ None, ADMIN_UPDATE_GUIDANCE));
    views.push(render_bottom_popup(&chat, /*width*/ 80));
    insta::assert_snapshot!("rc_menu", views.join("\n\n"));
}

#[tokio::test]
async fn rc_from_cloud_uses_only_the_parked_local_daemon() {
    use super::super::backend_sessions::BackendSessions;
    use super::super::test_support::make_test_app;
    use crate::AppServerTarget;
    let mut app = make_test_app().await;
    let (cloud, _cloud_commands, _cloud_events) = CloudClient::channel(/*capacity*/ 2);
    let cloud_session = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(cloud),
        ThreadParamsMode::Cloud,
    );
    let (local, _local_commands, local_events) = CloudClient::channel(/*capacity*/ 2);
    let local_id = BackendId::new();
    let local_session = AppServerSession::new(
        local_id,
        AppServerClient::Cloud(local),
        ThreadParamsMode::Embedded,
    );
    let mut backends = BackendSessions::from_initial(cloud_session, vec![local_session]);
    let mut settings = app.backend_settings();
    settings.target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: codex_utils_absolute_path::AbsolutePathBuf::relative_to_current_dir(
                "codex.sock",
            )
            .unwrap(),
        },
        allow_embedded_fallback: false,
    };
    backends.settings.insert(local_id, settings);
    app.app_server_target = AppServerTarget::Embedded;
    assert!(local_daemon(&app, &backends).is_none());
    app.app_server_target = AppServerTarget::Cloud;
    drop(local_events);
    assert_eq!(
        backends.next_event(/*paused_backend*/ None).await.0,
        local_id
    );
    assert!(!backends.is_listening(local_id));
    assert_eq!(
        local_daemon(&app, &backends).map(|(session, _)| session.backend_id()),
        Some(local_id)
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_mutation_reports_unknown_outcome() {
    let result = bounded(Action::Disable, std::future::pending()).await;
    assert_matches::assert_matches!(result, Outcome::Error(message) if message.contains("outcome is unknown"));
}
