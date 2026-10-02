//! Exercises overload deadlines and automatic reconnect backoff over real connections.
//! Auth reloads must respect the deadline; shutdown must remain prompt.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn rate_limited_enrollment_respects_retry_after() {
    assert_overload_retry_after("429 Too Many Requests", /*reject_enrollment*/ true).await;
}

#[tokio::test]
async fn rate_limited_websocket_respects_retry_after() {
    assert_overload_retry_after("429 Too Many Requests", /*reject_enrollment*/ false).await;
}

#[tokio::test]
async fn enrollment_resumes_after_retry_after() {
    assert_overload_retry_after("503 Service Unavailable", /*reject_enrollment*/ true).await;
}

#[tokio::test]
async fn websocket_resumes_after_retry_after() {
    assert_overload_retry_after("503 Service Unavailable", /*reject_enrollment*/ false).await;
}

async fn assert_overload_retry_after(status: &str, reject_enrollment: bool) {
    let verify_recovery = status == "503 Service Unavailable";
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, _transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let auth_manager = remote_control_auth_manager_with_home(&codex_home);
    let mut initial_auth = remote_control_auth_dot_json(Some("account_id"));
    initial_auth
        .tokens
        .as_mut()
        .expect("fixture should contain tokens")
        .access_token = "Initial Access Token".to_string();
    save_auth(
        codex_home.path(),
        &initial_auth,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("initial credentials should save");
    auth_manager.reload().await;
    let (remote_task, remote_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url: remote_control_url_for_listener(&listener),
            installation_id: TEST_INSTALLATION_ID.to_string(),
            policy: RemoteControlPolicy::Allowed,
        },
        Some(remote_control_state_runtime(&codex_home).await),
        auth_manager.clone(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        RemoteControlStartupMode::EnabledEphemeral,
    )
    .await
    .expect("remote control should start");
    let mut status_rx = remote_handle.status_receiver();
    let mut rejected_request = accept_http_request(&listener).await;
    assert_eq!(
        rejected_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    if !reject_enrollment {
        respond_with_json(
            rejected_request.stream,
            remote_control_server_token_response(
                "srv_e_test",
                "env_test",
                TEST_REMOTE_CONTROL_SERVER_TOKEN,
            ),
        )
        .await;
        rejected_request = accept_http_request(&listener).await;
        assert_eq!(
            rejected_request.request_line,
            "GET /backend-api/wham/remote/control/server HTTP/1.1"
        );
    }
    let response_started_at = std::time::Instant::now();
    respond_with_status_and_headers(
        rejected_request.stream,
        status,
        &[("Retry-After", if verify_recovery { "3" } else { "120" })],
        "overloaded",
    )
    .await;
    timeout(
        Duration::from_secs(5),
        status_rx.wait_for(|status| status.status == RemoteControlConnectionStatus::Errored),
    )
    .await
    .expect("the overload response should be processed")
    .expect("the status channel should remain open");

    let auth_changes = auth_manager.auth_change_receiver();
    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("account_id")),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("updated credentials should save");
    auth_manager.reload().await;
    assert!(
        auth_changes
            .has_changed()
            .expect("auth watch should remain open")
    );

    if !verify_recovery {
        let pairing_error = timeout(
            Duration::from_secs(1),
            remote_handle.start_pairing(
                RemoteControlPairingStartParams::default(),
                /*app_server_client_name*/ None,
            ),
        )
        .await
        .expect("pairing should report the active retry delay promptly")
        .expect_err("pairing must respect the shared server retry delay");
        let retry_delay = server_api::remote_control_retry_delay(&pairing_error)
            .expect("pairing should preserve the overload retry deadline");
        assert!(
            retry_delay >= Duration::from_secs(120).saturating_sub(response_started_at.elapsed()),
            "pairing must preserve the full remaining Retry-After interval"
        );
        assert!(
            timeout(Duration::from_secs(2), listener.accept())
                .await
                .is_err(),
            "no enrollment, refresh or handshake should retry before Retry-After ({status})"
        );
        remote_handle.disable_ephemeral().await;
        assert!(
            timeout(Duration::from_secs(2), listener.accept())
                .await
                .is_err(),
            "disabling remote control must stop connection attempts"
        );
        remote_handle
            .enable_ephemeral()
            .expect("remote control should enable again");
        assert!(
            timeout(Duration::from_secs(2), listener.accept())
                .await
                .is_err(),
            "re-enabling remote control must preserve the server retry delay"
        );
    }
    if !reject_enrollment {
        assert_eq!(
            remote_handle
                .inner
                .session()
                .current_enrollment
                .snapshot()
                .and_then(|enrollment| enrollment.remote_control_token),
            Some(TEST_REMOTE_CONTROL_SERVER_TOKEN.to_string()),
            "overload must preserve the enrolled token"
        );
    }
    if verify_recovery {
        remote_handle.disable_ephemeral().await;
        remote_handle
            .enable_ephemeral()
            .expect("remote control should enable again");
        let (stream, _) = timeout(Duration::from_secs(45), listener.accept())
            .await
            .expect("retry should resume after the server delay and bounded jitter")
            .expect("retry connection should succeed");
        assert!(
            response_started_at.elapsed() >= Duration::from_secs(3),
            "the retry must wait for the full Retry-After interval"
        );
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        timeout(Duration::from_secs(5), reader.read_line(&mut request_line))
            .await
            .expect("retry should send the request promptly")
            .expect("retry request should read");
        assert_eq!(
            request_line.trim_end(),
            if reject_enrollment {
                "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
            } else {
                "GET /backend-api/wham/remote/control/server HTTP/1.1"
            }
        );
    }
    shutdown_token.cancel();
    timeout(Duration::from_secs(1), remote_task)
        .await
        .expect("shutdown must interrupt the server retry delay")
        .expect("remote task should finish");
}

#[tokio::test]
async fn reconnect_backoff_covers_refresh_and_resets_after_healthy_connection() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, _transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, remote_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url: remote_control_url_for_listener(&listener),
            installation_id: TEST_INSTALLATION_ID.to_string(),
            policy: RemoteControlPolicy::Allowed,
        },
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        RemoteControlStartupMode::EnabledEphemeral,
    )
    .await
    .expect("remote control should start");
    let mut status_rx = remote_handle.status_receiver();
    let enroll_request = accept_http_request(&listener).await;
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut websocket = accept_remote_control_connection(&listener).await;
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connected)
        .await
        .expect("initial connection should succeed");

    // Make the next connection require token work before its handshake.
    remote_handle
        .inner
        .session()
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("enrollment should exist")
        .clear_server_token();
    let disconnected_at = tokio::time::Instant::now();
    websocket.close(None).await.expect("websocket should close");
    drop(websocket);
    let refresh_request =
        accept_http_request_with_timeout(&listener, Duration::from_secs(45)).await;
    assert_eq!(
        refresh_request.request_line,
        "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
    );
    assert!(
        disconnected_at.elapsed() >= Duration::from_millis(2_500),
        "the first automatic reconnect must wait before refreshing its token"
    );
    respond_with_json(
        refresh_request.stream,
        remote_control_server_token_response(
            "srv_e_test",
            "env_test",
            TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut websocket = accept_remote_control_connection(&listener).await;
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connected)
        .await
        .expect("reconnect should succeed");

    let disconnected_at = tokio::time::Instant::now();
    websocket.close(None).await.expect("websocket should close");
    drop(websocket);
    let mut websocket =
        accept_remote_control_connection_with_timeout(&listener, Duration::from_secs(45)).await;
    assert!(
        disconnected_at.elapsed() >= Duration::from_secs(5),
        "a short-lived connection must retain the increased backoff"
    );
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connected)
        .await
        .expect("reconnect should succeed");

    // Skip most of the healthy interval without crossing the 60-second pong timeout.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(45)).await;
    tokio::time::resume();
    // Drive the socket so tungstenite answers pings while we cross the reset threshold.
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(16)) => {}
        _ = async {
            while let Some(frame) = websocket.next().await {
                frame.expect("healthy connection should remain readable");
            }
        } => panic!("healthy connection should remain open"),
    }
    websocket.close(None).await.expect("websocket should close");
    drop(websocket);
    let mut websocket =
        accept_remote_control_connection_with_timeout(&listener, Duration::from_secs(8)).await;
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connected)
        .await
        .expect("reconnect should succeed");

    websocket.close(None).await.expect("websocket should close");
    drop(websocket);
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connecting)
        .await
        .expect("closed socket should enter reconnect backoff");
    remote_handle.disable_ephemeral().await;
    // Re-enable immediately, before the websocket task can observe the disabled state.
    remote_handle
        .enable_ephemeral()
        .expect("remote control should enable again");
    let mut websocket =
        accept_remote_control_connection_with_timeout(&listener, Duration::from_secs(2)).await;
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connected)
        .await
        .expect("explicit connection should succeed");
    websocket.close(None).await.expect("websocket should close");
    drop(websocket);
    status_rx
        .wait_for(|status| status.status == RemoteControlConnectionStatus::Connecting)
        .await
        .expect("closed socket should enter reconnect backoff");
    shutdown_token.cancel();
    timeout(Duration::from_secs(1), remote_task)
        .await
        .expect("shutdown must interrupt reconnect backoff")
        .expect("remote task should finish");
}
