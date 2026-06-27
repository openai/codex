use super::tests::TEST_HTTP_ACCEPT_TIMEOUT;
use super::tests::TEST_INSTALLATION_ID;
use super::tests::TEST_REMOTE_CONTROL_SERVER_TOKEN;
use super::tests::accept_http_request;
use super::tests::enabled_desired_state_sender;
use super::tests::remote_control_auth_manager;
use super::tests::remote_control_enrollment;
use super::tests::remote_control_state_runtime;
use super::tests::remote_control_status_channel;
use super::tests::remote_control_url_for_listener;
use super::tests::respond_with_status_and_headers;
use super::tests::test_current_enrollment;
use super::*;
use crate::transport::remote_control::protocol::normalize_remote_control_url;
use crate::transport::remote_control::tests::remote_control_handle_with_current_enrollment;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;

async fn connect_test_websocket(
    remote_control_target: &RemoteControlTarget,
    state_db: &StateRuntime,
    auth_manager: &Arc<AuthManager>,
    current_enrollment: &CurrentRemoteControlEnrollment,
) -> io::Result<()> {
    let mut auth_recovery = auth_manager.unauthorized_recovery();
    let mut auth_change_rx = auth_manager.auth_change_receiver();
    let (status_publisher, _) = remote_control_status_channel();
    let desired_state_tx = enabled_desired_state_sender();
    let desired_state_persistence_lock = Semaphore::new(1);
    connect_remote_control_websocket(
        remote_control_target,
        Some(state_db),
        RemoteControlAuthContext {
            auth_manager,
            auth_recovery: &mut auth_recovery,
            auth_change_rx: &mut auth_change_rx,
        },
        current_enrollment,
        RemoteControlConnectOptions {
            installation_id: TEST_INSTALLATION_ID,
            server_name: "test-server",
            subscribe_cursor: None,
            app_server_client_name: None,
            desired_state_tx: &desired_state_tx,
            desired_state_persistence_lock: &desired_state_persistence_lock,
        },
        &status_publisher,
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn proactive_refresh_failure_uses_valid_token_for_websocket_connect() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let server_task = tokio::spawn(async move {
        let (stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        respond_with_status_and_headers(stream, "502 Bad Gateway", &[], "upstream unavailable")
            .await;
        accept_test_websocket(&listener).await
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut enrollment = remote_control_enrollment(Some(TEST_REMOTE_CONTROL_SERVER_TOKEN));
    enrollment.expires_at = Some(time::OffsetDateTime::now_utc() + time::Duration::minutes(4));
    let current_enrollment = test_current_enrollment(Some(enrollment));

    let refresh_started_at = time::OffsetDateTime::now_utc();
    connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect("valid token should allow websocket connect after proactive refresh failure");
    let refresh_completed_at = time::OffsetDateTime::now_utc();
    let server_websocket = server_task.await.expect("server task should succeed");

    let enrollment = current_enrollment
        .lock()
        .await
        .clone()
        .expect("enrollment should remain available");
    assert_eq!(
        enrollment.remote_control_token.as_deref(),
        Some(TEST_REMOTE_CONTROL_SERVER_TOKEN)
    );
    let next_refresh_at = enrollment
        .next_refresh_at
        .expect("transient refresh should set a retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(24)
            ..=refresh_completed_at + time::Duration::seconds(36))
            .contains(&next_refresh_at)
    );
    drop(server_websocket);
}

#[tokio::test]
async fn proactive_refresh_connection_failure_uses_valid_token_for_websocket_connect() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let server_task = tokio::spawn(async move {
        let (stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        drop(stream);
        accept_test_websocket(&listener).await
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut enrollment = remote_control_enrollment(Some(TEST_REMOTE_CONTROL_SERVER_TOKEN));
    enrollment.expires_at = Some(time::OffsetDateTime::now_utc() + time::Duration::minutes(4));
    let current_enrollment = test_current_enrollment(Some(enrollment));

    connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect("valid token should allow websocket connect after refresh connection failure");
    let server_websocket = server_task.await.expect("server task should succeed");

    assert!(
        current_enrollment
            .snapshot()
            .and_then(|enrollment| enrollment.next_refresh_at)
            .is_some(),
        "connection failure should set a retry deadline"
    );
    drop(server_websocket);
}

#[tokio::test]
async fn websocket_retry_after_throttles_pairing_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let server_task = tokio::spawn(async move {
        let (stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        respond_with_status_and_headers(
            stream,
            "502 Bad Gateway",
            &[("retry-after", "120")],
            "upstream unavailable",
        )
        .await;
        let first_websocket = accept_test_websocket(&listener).await;
        let (pairing_stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        respond_with_status_and_headers(
            pairing_stream,
            "200 OK",
            &[],
            r#"{"pairing_code":"pairing-code","manual_pairing_code":"ABCD-EFGH","server_id":"srv_e_test","environment_id":"env_test","expires_at":"3026-05-22T12:34:56Z"}"#,
        )
        .await;
        first_websocket
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager.clone());
    remote_handle.state_db = Some(state_db.clone());
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(time::OffsetDateTime::now_utc() + time::Duration::minutes(4));
    let current_enrollment = remote_handle.current_enrollment.clone();
    let refresh_started_at = time::OffsetDateTime::now_utc();
    connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect("first websocket should connect after deferred refresh");
    let refresh_completed_at = time::OffsetDateTime::now_utc();
    let next_refresh_at = current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("Retry-After should set a retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(120)
            ..=refresh_completed_at + time::Duration::seconds(120))
            .contains(&next_refresh_at)
    );

    let pairing_response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("websocket Retry-After should throttle pairing refresh");
    let first_server_websocket = server_task.await.expect("server task should succeed");

    assert_eq!(
        pairing_response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    drop(first_server_websocket);
}

#[tokio::test]
async fn pairing_http_date_retry_after_throttles_websocket_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let retry_after =
        httpdate::fmt_http_date(std::time::SystemTime::now() + Duration::from_secs(120));
    let expected_next_refresh_at = time::OffsetDateTime::from(
        httpdate::parse_http_date(&retry_after).expect("Retry-After date should parse"),
    );
    let server_task = tokio::spawn(async move {
        let (refresh_stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        respond_with_status_and_headers(
            refresh_stream,
            "502 Bad Gateway",
            &[("retry-after", &retry_after)],
            "upstream unavailable",
        )
        .await;
        let (pairing_stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        respond_with_status_and_headers(
            pairing_stream,
            "200 OK",
            &[],
            r#"{"pairing_code":"pairing-code","manual_pairing_code":"ABCD-EFGH","server_id":"srv_e_test","environment_id":"env_test","expires_at":"3026-05-22T12:34:56Z"}"#,
        )
        .await;
        accept_test_websocket(&listener).await
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager.clone());
    remote_handle.state_db = Some(state_db.clone());
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(time::OffsetDateTime::now_utc() + time::Duration::minutes(4));
    let current_enrollment = remote_handle.current_enrollment.clone();

    let pairing_response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("pairing should continue after proactive refresh failure");
    assert_eq!(
        current_enrollment
            .snapshot()
            .and_then(|enrollment| enrollment.next_refresh_at),
        Some(expected_next_refresh_at)
    );
    connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect("pairing Retry-After should throttle websocket refresh");
    let server_websocket = server_task.await.expect("server task should succeed");

    assert_eq!(
        pairing_response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    drop(server_websocket);
}

async fn assert_refresh_failure_blocks_websocket(
    expires_in: time::Duration,
    response_delay: Duration,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let (connects_done_tx, connects_done_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move {
        let (stream, request_line) = accept_http_request(&listener).await;
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        tokio::time::sleep(response_delay).await;
        respond_with_status_and_headers(
            stream,
            "502 Bad Gateway",
            &[("retry-after", "120")],
            "upstream unavailable",
        )
        .await;
        assert_no_connection_until_connect_finishes(&listener, connects_done_rx).await;
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut enrollment = remote_control_enrollment(Some(TEST_REMOTE_CONTROL_SERVER_TOKEN));
    enrollment.expires_at = Some(time::OffsetDateTime::now_utc() + expires_in);
    let current_enrollment = test_current_enrollment(Some(enrollment));

    let refresh_started_at = time::OffsetDateTime::now_utc();
    let refresh_err = connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect_err("required refresh failure should block websocket connect");
    let refresh_completed_at = time::OffsetDateTime::now_utc();
    let deferred_err = connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect_err("required refresh deadline should block websocket reconnect");
    connects_done_tx
        .send(())
        .expect("server should wait for connect attempts to finish");

    server_task.await.expect("server task should succeed");
    assert!(refresh_err.to_string().contains("HTTP 502 Bad Gateway"));
    assert_eq!(deferred_err.kind(), io::ErrorKind::WouldBlock);
    assert!(deferred_err.to_string().contains("refresh deferred until"));
    let next_refresh_at = current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("required refresh failure should set a retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(120)
            ..=refresh_completed_at + time::Duration::seconds(120))
            .contains(&next_refresh_at)
    );
}

#[tokio::test]
async fn expired_token_refresh_failure_throttles_reconnect_without_websocket() {
    assert_refresh_failure_blocks_websocket(-time::Duration::seconds(1), Duration::ZERO).await;
}

#[tokio::test]
async fn token_expiring_during_refresh_failure_throttles_reconnect_without_websocket() {
    assert_refresh_failure_blocks_websocket(
        time::Duration::seconds(1),
        Duration::from_millis(1_200),
    )
    .await;
}

#[tokio::test]
async fn websocket_auth_failure_does_not_clear_rotated_server_token() {
    let attempted_enrollment = remote_control_enrollment(Some("old-token"));
    let mut rotated_enrollment = attempted_enrollment.clone();
    rotated_enrollment.remote_control_token = Some("new-token".to_string());
    rotated_enrollment.expires_at =
        Some(time::OffsetDateTime::now_utc() + time::Duration::hours(1));
    let current_enrollment = test_current_enrollment(Some(rotated_enrollment.clone()));

    clear_remote_control_server_token_if_matches(&current_enrollment, &attempted_enrollment)
        .await
        .expect("matching enrollment identity should remain available");

    assert_eq!(current_enrollment.snapshot(), Some(rotated_enrollment));
}

async fn accept_test_websocket(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = timeout(TEST_HTTP_ACCEPT_TIMEOUT, listener.accept())
        .await
        .expect("websocket request should arrive in time")
        .expect("listener accept should succeed");
    accept_async(stream)
        .await
        .expect("websocket handshake should succeed")
}

async fn assert_no_connection_until_connect_finishes(
    listener: &TcpListener,
    mut connect_done_rx: oneshot::Receiver<()>,
) {
    tokio::select! {
        accepted = listener.accept() => {
            accepted.expect("unexpected websocket connection should be accepted");
            panic!("required refresh failure must not proceed to websocket connect");
        }
        connect_done = &mut connect_done_rx => {
            connect_done.expect("connect completion should be reported");
        }
    }
}
