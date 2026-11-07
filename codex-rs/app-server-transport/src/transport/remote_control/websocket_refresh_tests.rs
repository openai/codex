use super::tests::TEST_HTTP_ACCEPT_TIMEOUT;
use super::tests::TEST_INSTALLATION_ID;
use super::tests::TEST_REMOTE_CONTROL_SERVER_TOKEN;
use super::tests::accept_http_request;
use super::tests::enabled_desired_state_sender;
use super::tests::remote_control_auth_dot_json;
use super::tests::remote_control_auth_manager;
use super::tests::remote_control_enrollment;
use super::tests::remote_control_state_runtime;
use super::tests::remote_control_status_channel;
use super::tests::remote_control_url_for_listener;
use super::tests::respond_with_status_and_headers;
use super::tests::test_current_enrollment;
use super::*;
use crate::transport::remote_control::protocol::normalize_remote_control_url;
use crate::transport::remote_control::server_api::remote_control_retry_at;
use crate::transport::remote_control::tests::remote_control_handle_with_current_enrollment;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::save_auth;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
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
    let session_auth = RemoteControlAuth::capture(auth_manager.clone(), /*runtime_auth*/ None).0;
    let mut auth_recovery = session_auth.unauthorized_recovery();
    let mut auth_change_rx = auth_manager.auth_change_receiver();
    let (status_publisher, _) = remote_control_status_channel();
    let desired_state_tx = enabled_desired_state_sender();
    let persistence = RemoteControlPersistence::default();
    connect_remote_control_websocket(
        remote_control_target,
        Some(state_db),
        RemoteControlAuthContext {
            auth_manager: &session_auth,
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
            persistence: &persistence,
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
        listener
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
    let refresh_error = connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    )
    .await
    .expect_err("an explicit server deadline must defer the handshake even with a valid token");
    let refresh_completed_at = time::OffsetDateTime::now_utc();
    let next_refresh_at = current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("Retry-After should set a retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(120)
            ..=refresh_completed_at + time::Duration::seconds(150))
            .contains(&next_refresh_at)
    );

    let pairing_error = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("the refresh deadline must also defer pairing");
    let listener = server_task.await.expect("server task should succeed");
    assert_eq!(
        remote_control_retry_at(&refresh_error),
        Some(next_refresh_at)
    );
    assert_eq!(
        remote_control_retry_at(&pairing_error),
        Some(next_refresh_at)
    );
    assert_eq!(
        current_enrollment
            .snapshot()
            .and_then(|enrollment| enrollment.remote_control_token),
        Some(TEST_REMOTE_CONTROL_SERVER_TOKEN.to_string())
    );
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("no handshake or pairing should bypass the proactive refresh deadline");
}

#[tokio::test]
async fn pairing_http_date_retry_after_throttles_websocket_refresh() {
    for status in [
        "429 Too Many Requests",
        "503 Service Unavailable",
        "502 Bad Gateway",
    ] {
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
                status,
                &[("retry-after", &retry_after)],
                "upstream unavailable",
            )
            .await;
            listener
        });
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let auth_manager = remote_control_auth_manager();
        let mut remote_handle = remote_control_handle_with_current_enrollment(
            &remote_control_url,
            auth_manager.clone(),
        );
        remote_handle.state_db = Some(state_db.clone());
        remote_handle
            .current_enrollment
            .lock()
            .await
            .as_mut()
            .expect("current enrollment should exist")
            .expires_at = Some(time::OffsetDateTime::now_utc() + time::Duration::minutes(4));
        let current_enrollment = remote_handle.current_enrollment.clone();

        let refresh_error = remote_handle
            .start_pairing(
                RemoteControlPairingStartParams::default(),
                /*app_server_client_name*/ None,
            )
            .await
            .expect_err("an explicit server deadline must defer pairing even with a valid token");
        let listener = server_task.await.expect("server task should succeed");
        let retry_at = remote_control_retry_at(&refresh_error)
            .expect("the proactive refresh response should preserve its deadline");
        let enrollment = current_enrollment
            .snapshot()
            .expect("enrollment should remain");
        assert_eq!(enrollment.next_refresh_at, Some(retry_at));
        assert_eq!(
            enrollment.remote_control_token.as_deref(),
            Some(TEST_REMOTE_CONTROL_SERVER_TOKEN)
        );
        assert!(
            (expected_next_refresh_at..=expected_next_refresh_at + time::Duration::seconds(30))
                .contains(&retry_at)
        );
        let connect_error = connect_test_websocket(
            &remote_control_target,
            state_db.as_ref(),
            &auth_manager,
            &current_enrollment,
        )
        .await
        .expect_err("the refresh deadline must also defer the handshake");
        assert_eq!(remote_control_retry_at(&connect_error), Some(retry_at));
        let pairing_error = remote_handle
            .start_pairing(
                RemoteControlPairingStartParams::default(),
                /*app_server_client_name*/ None,
            )
            .await
            .expect_err("another pairing request must retain the same deadline");
        assert_eq!(remote_control_retry_at(&pairing_error), Some(retry_at));
        timeout(Duration::from_millis(100), listener.accept())
            .await
            .expect_err("no pairing, refresh or handshake should bypass the deadline");
    }
}

#[tokio::test]
async fn pairing_during_pending_handshake_respects_later_overload() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = remote_control_auth_manager();
    let mut remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager.clone());
    remote_handle.state_db = Some(state_db.clone());
    let current_enrollment = remote_handle.current_enrollment.clone();
    let connect = connect_test_websocket(
        &remote_control_target,
        state_db.as_ref(),
        &auth_manager,
        &current_enrollment,
    );
    tokio::pin!(connect);
    let (handshake_stream, request_line) = tokio::select! {
        result = &mut connect => panic!("handshake should wait for its response: {result:?}"),
        request = accept_http_request(&listener) => request,
    };
    assert_eq!(
        request_line,
        "GET /backend-api/wham/remote/control/server HTTP/1.1"
    );

    let pairing = remote_handle.start_pairing(
        RemoteControlPairingStartParams::default(),
        /*app_server_client_name*/ None,
    );
    tokio::pin!(pairing);
    let (pairing_stream, request_line) = tokio::select! {
        result = &mut pairing => panic!("pairing should wait for its response: {result:?}"),
        request = accept_http_request(&listener) => request,
    };
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
    timeout(Duration::from_secs(1), pairing)
        .await
        .expect("pairing must stay responsive while the handshake is pending")
        .expect("pairing should succeed before any overload is observed");
    respond_with_status_and_headers(
        handshake_stream,
        "503 Service Unavailable",
        &[("Retry-After", "120")],
        "overloaded",
    )
    .await;
    let connect_error = connect
        .await
        .expect_err("the handshake should report overload");
    let retry_at = remote_control_retry_at(&connect_error)
        .expect("the handshake should preserve its retry deadline");
    let pairing_error = timeout(
        Duration::from_secs(1),
        remote_handle.start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        ),
    )
    .await
    .expect("new pairing should report the deadline promptly")
    .expect_err("new pairing must honor the handshake deadline");
    assert_eq!(remote_control_retry_at(&pairing_error), Some(retry_at));
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("new pairing must not bypass the newly recorded deadline");
}

#[tokio::test]
async fn pairing_auth_recovery_respects_concurrent_handshake_overload() {
    for (pairing_status, auth_endpoint) in
        [("401 Unauthorized", "refresh"), ("404 Not Found", "enroll")]
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = remote_control_url_for_listener(&listener);
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let codex_home = TempDir::new().expect("temp dir should create");
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("stale-token"),
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )
        .expect("stale auth should save");
        let auth_manager = AuthManager::shared(
            codex_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            codex_login::test_support::transport_default_auth_route_config(),
        )
        .await;
        let state_db = remote_control_state_runtime(&codex_home).await;
        let mut remote_handle = remote_control_handle_with_current_enrollment(
            &remote_control_url,
            auth_manager.clone(),
        );
        remote_handle.state_db = Some(state_db.clone());
        let current_enrollment = remote_handle.current_enrollment.clone();
        let connect = connect_test_websocket(
            &remote_control_target,
            state_db.as_ref(),
            &auth_manager,
            &current_enrollment,
        );
        tokio::pin!(connect);
        let (handshake_stream, request_line) = tokio::select! {
            result = &mut connect => panic!("handshake should wait for its response: {result:?}"),
            request = accept_http_request(&listener) => request,
        };
        assert_eq!(
            request_line,
            "GET /backend-api/wham/remote/control/server HTTP/1.1"
        );

        let pairing = remote_handle.start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        );
        tokio::pin!(pairing);
        let (pairing_stream, request_line) = tokio::select! {
            result = &mut pairing => panic!("pairing should wait for its response: {result:?}"),
            request = accept_http_request(&listener) => request,
        };
        assert_eq!(
            request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        respond_with_status_and_headers(pairing_stream, pairing_status, &[], "retry auth").await;
        let (auth_stream, request_line) = tokio::select! {
            result = &mut pairing => panic!("auth should wait for its response: {result:?}"),
            request = accept_http_request(&listener) => request,
        };
        assert_eq!(
            request_line,
            format!("POST /backend-api/wham/remote/control/server/{auth_endpoint} HTTP/1.1")
        );

        respond_with_status_and_headers(
            handshake_stream,
            "503 Service Unavailable",
            &[("Retry-After", "120")],
            "overloaded",
        )
        .await;
        let connect_error = connect
            .await
            .expect_err("the handshake should report overload");
        let retry_at = remote_control_retry_at(&connect_error)
            .expect("the handshake should preserve its retry deadline");
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("fresh-token"),
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )
        .expect("replacement auth should save");
        respond_with_status_and_headers(auth_stream, "401 Unauthorized", &[], "stale auth").await;
        let pairing_error = timeout(TEST_HTTP_ACCEPT_TIMEOUT, pairing)
            .await
            .expect("auth recovery should report the deadline promptly")
            .expect_err("auth recovery must honor the handshake deadline");
        assert_eq!(remote_control_retry_at(&pairing_error), Some(retry_at));
        timeout(Duration::from_millis(100), listener.accept())
            .await
            .expect_err("auth recovery must not send another request during overload");
    }
}

#[tokio::test]
async fn pairing_overload_defers_pairing_and_websocket_requests() {
    for status in ["429 Too Many Requests", "503 Service Unavailable"] {
        for check_status in [false, true] {
            for incomplete_body in [false, true] {
                let listener = TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("listener should bind");
                let remote_control_url = remote_control_url_for_listener(&listener);
                let remote_control_target =
                    normalize_remote_control_url(&remote_control_url).expect("target should parse");
                let server_task = tokio::spawn(async move {
                    let (mut stream, request_line) = accept_http_request(&listener).await;
                    assert_eq!(
                        request_line,
                        if check_status {
                            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
                        } else {
                            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
                        }
                    );
                    if incomplete_body {
                        stream.write_all(format!(
                            "HTTP/1.1 {status}\r\nContent-Length: 100\r\nRetry-After: 120\r\nConnection: close\r\n\r\npartial"
                        ).as_bytes()).await.expect("partial response should send");
                        stream.shutdown().await.expect("response should close");
                    } else {
                        respond_with_status_and_headers(
                            stream,
                            status,
                            &[("Retry-After", "120")],
                            "overloaded",
                        )
                        .await;
                    }
                    listener
                });
                let codex_home = TempDir::new().expect("temp dir should create");
                let state_db = remote_control_state_runtime(&codex_home).await;
                let auth_manager = remote_control_auth_manager();
                let mut remote_handle = remote_control_handle_with_current_enrollment(
                    &remote_control_url,
                    auth_manager.clone(),
                );
                remote_handle.state_db = Some(state_db.clone());
                let current_enrollment = remote_handle.current_enrollment.clone();
                let status_params = || RemoteControlPairingStatusParams {
                    pairing_code: Some("pairing-code".to_string()),
                    manual_pairing_code: None,
                };
                let response = if check_status {
                    remote_handle
                        .pairing_status(status_params())
                        .await
                        .map(|_| ())
                } else {
                    remote_handle
                        .start_pairing(
                            RemoteControlPairingStartParams::default(),
                            /*app_server_client_name*/ None,
                        )
                        .await
                        .map(|_| ())
                };
                let error = response.expect_err("pairing should report overload");
                let listener = server_task.await.expect("server task should succeed");
                let retry_at = remote_control_retry_at(&error)
                    .expect("even a partial response must preserve the retry deadline");
                let pairing_error = remote_handle
                    .start_pairing(
                        RemoteControlPairingStartParams::default(),
                        /*app_server_client_name*/ None,
                    )
                    .await
                    .expect_err("pairing must wait for the server deadline");
                let status_error = remote_handle
                    .pairing_status(status_params())
                    .await
                    .expect_err("pairing status must wait for the server deadline");
                let connect_error = connect_test_websocket(
                    &remote_control_target,
                    state_db.as_ref(),
                    &auth_manager,
                    &current_enrollment,
                )
                .await
                .expect_err("the handshake must wait for the same deadline");
                for error in [pairing_error, status_error, connect_error] {
                    assert_eq!(remote_control_retry_at(&error), Some(retry_at));
                }
                assert_eq!(
                    current_enrollment
                        .snapshot()
                        .and_then(|enrollment| enrollment.remote_control_token),
                    Some(TEST_REMOTE_CONTROL_SERVER_TOKEN.to_string()),
                );
                timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .expect_err("no remote-control request should bypass the pairing deadline");
            }
        }
    }
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
    let retry_at = remote_control_retry_at(&refresh_err)
        .expect("the overload response should retain its retry deadline");
    assert_eq!(remote_control_retry_at(&deferred_err), Some(retry_at));
    let next_refresh_at = current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("required refresh failure should set a retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(120)
            ..=refresh_completed_at + time::Duration::seconds(150))
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
