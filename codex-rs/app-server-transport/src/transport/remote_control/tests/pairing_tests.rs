use super::super::protocol::RemoteControlPairingStatusRequest;
use super::super::protocol::StartRemoteControlPairingRequest;
use super::*;
use codex_login::AuthKeyringBackendKind;
use pretty_assertions::assert_eq;
use std::io;

fn remote_control_enrollment(
    remote_control_url: &str,
    remote_control_token: &str,
) -> RemoteControlEnrollment {
    RemoteControlEnrollment {
        remote_control_target: normalize_remote_control_url(remote_control_url)
            .expect("target should normalize"),
        account_id: "account-id".to_string(),
        environment_id: "environment-id".to_string(),
        server_id: "server-id".to_string(),
        server_name: "server-name".to_string(),
        remote_control_token: Some(remote_control_token.to_string()),
        expires_at: Some(
            OffsetDateTime::from_unix_timestamp(33_336_362_096)
                .expect("future timestamp should parse"),
        ),
        next_refresh_at: None,
    }
}

async fn auth_manager_with_replacement(
    codex_home: &TempDir,
    replacement_account_id: &str,
) -> Arc<AuthManager> {
    let mut stale_auth = remote_control_auth_dot_json(Some("account_id"));
    stale_auth
        .tokens
        .as_mut()
        .expect("stale auth should include tokens")
        .access_token = "stale-token".to_string();
    save_auth(
        codex_home.path(),
        &stale_auth,
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
        /*auth_route_config*/ None,
    )
    .await;
    let mut replacement_auth = remote_control_auth_dot_json(Some(replacement_account_id));
    replacement_auth
        .tokens
        .as_mut()
        .expect("replacement auth should include tokens")
        .access_token = "fresh-token".to_string();
    save_auth(
        codex_home.path(),
        &replacement_auth,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("replacement auth should save");
    auth_manager
}

fn pairing_response_json(server_id: &str, environment_id: &str) -> serde_json::Value {
    json!({
        "pairing_code": "pairing-code",
        "manual_pairing_code": "ABCD-EFGH",
        "server_id": server_id,
        "environment_id": environment_id,
        "expires_at": "3026-05-22T12:34:56Z",
    })
}

fn pairing_response(environment_id: &str) -> RemoteControlPairingStartResponse {
    RemoteControlPairingStartResponse {
        pairing_code: "pairing-code".to_string(),
        manual_pairing_code: Some("ABCD-EFGH".to_string()),
        environment_id: environment_id.to_string(),
        expires_at: 33_336_362_096,
    }
}

async fn pairing_error(status: &'static str, body: &'static str) -> (String, String) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let expected_pair_url = normalize_remote_control_url(&remote_control_url)
        .expect("target should normalize")
        .pair_url;
    let server_task = tokio::spawn(async move {
        let pairing_request = accept_http_request(&listener).await;
        respond_with_status_and_headers(
            pairing_request.stream,
            status,
            &[("x-request-id", "request-123"), ("cf-ray", "ray-123")],
            body,
        )
        .await;
    });

    let err = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .start_pairing(StartRemoteControlPairingRequest { manual_code: false })
        .await
        .expect_err("pairing should fail");
    server_task.await.expect("server task should finish");
    (err.to_string(), expected_pair_url)
}

async fn pairing_response_error(body: serde_json::Value) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let pairing_request = accept_http_request(&listener).await;
        respond_with_json(pairing_request.stream, body).await;
    });

    let err = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .start_pairing(StartRemoteControlPairingRequest { manual_code: false })
        .await
        .expect_err("pairing should fail");
    server_task.await.expect("server task should finish");
    err.to_string()
}

async fn pairing_status_error(status: &'static str, body: &'static str) -> (io::Error, String) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let expected_status_url = normalize_remote_control_url(&remote_control_url)
        .expect("target should normalize")
        .pair_status_url;
    let server_task = tokio::spawn(async move {
        let status_request = accept_http_request(&listener).await;
        respond_with_status_and_headers(
            status_request.stream,
            status,
            &[("x-request-id", "request-123"), ("cf-ray", "ray-123")],
            body,
        )
        .await;
    });

    let err = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .pairing_status(RemoteControlPairingStatusRequest {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await
        .expect_err("pairing status should fail");
    server_task.await.expect("server task should finish");
    (err, expected_status_url)
}

#[tokio::test]
async fn remote_control_handle_starts_pairing_before_websocket_connects() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&refresh_request.body)
                .expect("refresh request body should deserialize"),
            json!({
                "server_id": "srv_e_test",
                "installation_id": TEST_INSTALLATION_ID,
            })
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

        let pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            pairing_request.headers.get("authorization"),
            Some(&format!(
                "Bearer {TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN}"
            ))
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&pairing_request.body)
                .expect("pairing request body should deserialize"),
            json!({ "manual_code": true })
        );
        respond_with_json(
            pairing_request.stream,
            pairing_response_json("srv_e_test", "env_test"),
        )
        .await;
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() + time::Duration::seconds(29));

    let response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams { manual_code: true },
            /*app_server_client_name*/ None,
        )
        .await
        .expect("pairing should use the current server before websocket connect");
    server_task.await.expect("server task should finish");

    assert_eq!(response, pairing_response("env_test"));
}

#[tokio::test]
async fn proactive_refresh_rate_limit_uses_valid_token_for_pairing() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        respond_with_status_and_headers(
            refresh_request.stream,
            "429 Too Many Requests",
            &[],
            "rate limited",
        )
        .await;

        let pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            pairing_request.headers.get("authorization"),
            Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
        );
        respond_with_json(
            pairing_request.stream,
            pairing_response_json("srv_e_test", "env_test"),
        )
        .await;
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() + time::Duration::minutes(4));

    let response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("valid token should allow pairing after proactive refresh failure");
    server_task.await.expect("server task should finish");

    assert_eq!(response, pairing_response("env_test"));
    assert!(
        remote_handle
            .current_enrollment
            .snapshot()
            .and_then(|enrollment| enrollment.next_refresh_at)
            .is_some()
    );
}

#[tokio::test]
async fn required_refresh_deadline_blocks_pairing_without_request() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        respond_with_status_and_headers(
            refresh_request.stream,
            "502 Bad Gateway",
            &[("retry-after", "120")],
            "upstream unavailable",
        )
        .await;
        listener
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() - time::Duration::seconds(1));

    let refresh_err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("required refresh failure should block pairing");
    let listener = server_task.await.expect("server task should finish");
    let next_refresh_at = remote_handle
        .current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("required pairing refresh should preserve the retry deadline");
    let deferred_err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("required refresh deadline should block pairing");

    assert!(refresh_err.to_string().contains("HTTP 502 Bad Gateway"));
    assert_eq!(deferred_err.kind(), io::ErrorKind::WouldBlock);
    assert!(
        deferred_err
            .to_string()
            .contains(&next_refresh_at.to_string())
    );
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("pairing should not issue a request before the refresh deadline");
}

#[tokio::test]
async fn remote_control_pairing_status_returns_pending() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let status_request = accept_http_request(&listener).await;
        assert_eq!(
            status_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
        );
        assert_eq!(
            status_request.headers.get("authorization"),
            Some(&"Bearer remote-control-token".to_string())
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&status_request.body)
                .expect("status request body should deserialize"),
            json!({ "pairing_code": "pairing-code" })
        );
        respond_with_json(status_request.stream, json!({ "claimed": false })).await;
    });

    let response = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .pairing_status(RemoteControlPairingStatusRequest {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await
        .expect("pairing status should succeed");
    server_task.await.expect("server task should finish");

    assert!(!response.claimed);
}

#[tokio::test]
async fn remote_control_pairing_status_accepts_manual_pairing_code() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let status_request = accept_http_request(&listener).await;
        assert_eq!(
            status_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&status_request.body)
                .expect("status request body should deserialize"),
            json!({ "manual_pairing_code": "ABCD-EFGH" })
        );
        respond_with_json(status_request.stream, json!({ "claimed": false })).await;
    });

    let response = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .pairing_status(RemoteControlPairingStatusRequest {
            pairing_code: None,
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
        })
        .await
        .expect("pairing status should succeed");
    server_task.await.expect("server task should finish");

    assert!(!response.claimed);
}

#[tokio::test]
async fn remote_control_pairing_status_returns_claimed() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let status_request = accept_http_request(&listener).await;
        assert_eq!(
            status_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
        );
        respond_with_json(status_request.stream, json!({ "claimed": true })).await;
    });

    let response = remote_control_enrollment(&remote_control_url, "remote-control-token")
        .pairing_status(RemoteControlPairingStatusRequest {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await
        .expect("pairing status should succeed");
    server_task.await.expect("server task should finish");

    assert!(response.claimed);
}

#[tokio::test]
async fn remote_control_handle_refreshes_after_pairing_status_auth_failure() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_status_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_status_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
        );
        assert_eq!(
            stale_status_request.headers.get("authorization"),
            Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
        );
        respond_with_status(stale_status_request.stream, "401 Unauthorized", "").await;

        let refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
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

        let refreshed_status_request = accept_http_request(&listener).await;
        assert_eq!(
            refreshed_status_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair/status HTTP/1.1"
        );
        assert_eq!(
            refreshed_status_request.headers.get("authorization"),
            Some(&format!(
                "Bearer {TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN}"
            ))
        );
        respond_with_json(refreshed_status_request.stream, json!({ "claimed": true })).await;
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );

    let response = remote_handle
        .pairing_status(RemoteControlPairingStatusParams {
            pairing_code: Some("pairing-code".to_string()),
            manual_pairing_code: None,
        })
        .await
        .expect("pairing status should refresh after server token auth failure");
    server_task.await.expect("server task should finish");

    assert!(response.claimed);
}

#[tokio::test]
async fn remote_control_pairing_status_maps_user_actionable_backend_errors() {
    for (status, expected_kind) in [
        ("403 Forbidden", io::ErrorKind::PermissionDenied),
        ("404 Not Found", io::ErrorKind::InvalidInput),
        ("410 Gone", io::ErrorKind::InvalidInput),
    ] {
        let (err, _expected_status_url) = pairing_status_error(status, "not available").await;
        assert_eq!(err.kind(), expected_kind);
    }
}

#[tokio::test]
async fn remote_control_pairing_status_preserves_decode_error_context() {
    let (err, expected_status_url) = pairing_status_error("200 OK", "{").await;
    let err = err.to_string();

    assert!(err.contains(&format!(
        "failed to parse remote control pairing status response from `{expected_status_url}`: HTTP 200 OK"
    )));
    assert!(err.contains("request-id: request-123"));
    assert!(err.contains("cf-ray: ray-123"));
    assert!(err.contains("body: {"));
    assert!(err.contains("decode error:"));
}

#[tokio::test]
async fn remote_control_handle_refreshes_after_pairing_auth_failure() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            stale_pairing_request.headers.get("authorization"),
            Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
        );
        respond_with_status(stale_pairing_request.stream, "401 Unauthorized", "").await;

        let refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        assert_eq!(
            refresh_request.headers.get("authorization"),
            Some(&"Bearer Access Token".to_string())
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

        let refreshed_pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            refreshed_pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            refreshed_pairing_request.headers.get("authorization"),
            Some(&format!(
                "Bearer {TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN}"
            ))
        );
        respond_with_json(
            refreshed_pairing_request.stream,
            pairing_response_json("srv_e_test", "env_test"),
        )
        .await;
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );

    let response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("pairing should refresh after server token auth failure");
    server_task.await.expect("server task should finish");

    assert_eq!(response, pairing_response("env_test"));
}

#[tokio::test]
async fn pairing_auth_failure_preserves_refresh_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        respond_with_status(pairing_request.stream, "401 Unauthorized", "").await;
    });
    let remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager(),
    );
    let next_refresh_at = OffsetDateTime::now_utc() + time::Duration::minutes(2);
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .next_refresh_at = Some(next_refresh_at);
    let mut expected_enrollment = remote_handle
        .current_enrollment
        .snapshot()
        .expect("current enrollment should exist");
    expected_enrollment.clear_server_token();

    let err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("refresh deadline should throttle recovery after token rejection");
    server_task.await.expect("server task should finish");

    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(
        remote_handle.current_enrollment.snapshot(),
        Some(expected_enrollment)
    );
}

#[tokio::test]
async fn remote_control_handle_recovers_auth_before_refreshing_pairing() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        assert_eq!(
            stale_refresh_request.headers.get("authorization"),
            Some(&"Bearer stale-token".to_string())
        );
        respond_with_status(stale_refresh_request.stream, "401 Unauthorized", "").await;

        let recovered_refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            recovered_refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        assert_eq!(
            recovered_refresh_request.headers.get("authorization"),
            Some(&"Bearer fresh-token".to_string())
        );
        respond_with_json(
            recovered_refresh_request.stream,
            remote_control_server_token_response(
                "srv_e_test",
                "env_test",
                TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
            ),
        )
        .await;

        let pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            pairing_request.headers.get("authorization"),
            Some(&format!(
                "Bearer {TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN}"
            ))
        );
        respond_with_json(
            pairing_request.stream,
            pairing_response_json("srv_e_test", "env_test"),
        )
        .await;
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let auth_manager = auth_manager_with_replacement(&codex_home, "account_id").await;
    let remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager);
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() + time::Duration::seconds(29));

    let response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("pairing should refresh after auth recovery");
    server_task.await.expect("server task should finish");

    assert_eq!(response, pairing_response("env_test"));
}

#[tokio::test]
async fn pairing_publishes_refresh_deferral_after_auth_recovery() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_refresh_request.headers.get("authorization"),
            Some(&"Bearer stale-token".to_string())
        );
        respond_with_status(stale_refresh_request.stream, "401 Unauthorized", "").await;

        let recovered_refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            recovered_refresh_request.headers.get("authorization"),
            Some(&"Bearer fresh-token".to_string())
        );
        respond_with_status_and_headers(
            recovered_refresh_request.stream,
            "502 Bad Gateway",
            &[("retry-after", "120")],
            "upstream unavailable",
        )
        .await;
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let auth_manager = auth_manager_with_replacement(&codex_home, "account_id").await;
    let remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager);
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() - time::Duration::seconds(1));

    let refresh_started_at = OffsetDateTime::now_utc();
    let refresh_err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("required refresh should remain strict after auth recovery");
    let refresh_completed_at = OffsetDateTime::now_utc();
    let deferred_err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("published deadline should throttle the next pairing refresh");
    server_task.await.expect("server task should finish");

    assert!(refresh_err.to_string().contains("HTTP 502 Bad Gateway"));
    assert_eq!(deferred_err.kind(), io::ErrorKind::WouldBlock);
    let next_refresh_at = remote_handle
        .current_enrollment
        .snapshot()
        .and_then(|enrollment| enrollment.next_refresh_at)
        .expect("required refresh failure should publish its retry deadline");
    assert!(
        (refresh_started_at + time::Duration::seconds(120)
            ..=refresh_completed_at + time::Duration::seconds(120))
            .contains(&next_refresh_at)
    );
}

#[tokio::test]
async fn pairing_auth_recovery_failure_publishes_cleared_server_token() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_refresh_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_refresh_request.request_line,
            "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
        );
        assert_eq!(
            stale_refresh_request.headers.get("authorization"),
            Some(&"Bearer stale-token".to_string())
        );
        respond_with_status(stale_refresh_request.stream, "401 Unauthorized", "").await;
    });
    let codex_home = TempDir::new().expect("temp dir should create");
    let auth_manager = auth_manager_with_replacement(&codex_home, "different_account_id").await;
    let remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager);
    remote_handle
        .current_enrollment
        .lock()
        .await
        .as_mut()
        .expect("current enrollment should exist")
        .expires_at = Some(OffsetDateTime::now_utc() + time::Duration::seconds(29));
    let mut expected_enrollment = remote_handle
        .current_enrollment
        .snapshot()
        .expect("current enrollment should exist");
    expected_enrollment.clear_server_token();

    let err = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect_err("pairing should fail after auth changes account");
    server_task.await.expect("server task should finish");

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        remote_handle.current_enrollment.snapshot(),
        Some(expected_enrollment)
    );
}

#[tokio::test]
async fn start_remote_control_pairing_preserves_backend_error_context() {
    let (err, expected_pair_url) =
        pairing_error("503 Service Unavailable", "pairing unavailable").await;

    assert_eq!(
        err,
        format!(
            "remote control pairing failed at `{expected_pair_url}`: HTTP 503 Service Unavailable, request-id: request-123, cf-ray: ray-123, body: pairing unavailable"
        )
    );
}

#[tokio::test]
async fn start_remote_control_pairing_preserves_decode_error_context() {
    let (err, expected_pair_url) = pairing_error("200 OK", "{").await;
    assert!(err.contains(&format!(
        "failed to parse remote control pairing response from `{expected_pair_url}`: HTTP 200 OK"
    )));
    assert!(err.contains("request-id: request-123"));
    assert!(err.contains("cf-ray: ray-123"));
    assert!(err.contains("body: {"));
    assert!(err.contains("decode error:"));
}

#[tokio::test]
async fn start_remote_control_pairing_rejects_mismatched_backend_enrollment() {
    assert_eq!(
        pairing_response_error(json!({
            "pairing_code": "pairing-code",
            "manual_pairing_code": "ABCD-EFGH",
            "server_id": "other-server-id",
            "environment_id": "other-environment-id",
            "expires_at": "3026-05-22T12:34:56Z",
        }))
        .await,
        "remote control pairing returned mismatched enrollment: expected server_id=server-id, environment_id=environment-id; got server_id=other-server-id, environment_id=other-environment-id"
    );
}

#[tokio::test]
async fn start_remote_control_pairing_preserves_expiry_parse_error_context() {
    let err = pairing_response_error(json!({
        "pairing_code": "pairing-code",
        "manual_pairing_code": "ABCD-EFGH",
        "server_id": "server-id",
        "environment_id": "environment-id",
        "expires_at": "not-a-timestamp",
    }))
    .await;

    assert!(err.contains("failed to parse remote control pairing response"));
    assert!(err.contains("HTTP 200 OK"));
    assert!(err.contains("request-id: <none>"));
    assert!(err.contains("cf-ray: <none>"));
    assert!(err.contains("\"expires_at\":\"not-a-timestamp\""));
    assert!(err.contains("expires_at parse error:"));
}

#[tokio::test]
async fn remote_control_handle_disable_keeps_current_enrollment() {
    let remote_handle = remote_control_handle_with_current_enrollment(
        TEST_REMOTE_CONTROL_URL,
        remote_control_auth_manager(),
    );

    remote_handle
        .desired_state_tx
        .send_replace(RemoteControlDesiredState::Disabled);
    assert!(
        remote_handle.current_enrollment.lock().await.is_some(),
        "disabled remote control should keep the selected pairing server"
    );
}

#[tokio::test]
async fn remote_control_handle_reenrolls_after_stale_pairing_enrollment() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let mut remote_handle = remote_control_handle_with_current_enrollment(
        &remote_control_url,
        remote_control_auth_manager_with_home(&codex_home),
    );
    remote_handle.state_db = Some(state_db.clone());
    let stale_enrollment = remote_handle
        .current_enrollment
        .lock()
        .await
        .clone()
        .expect("current enrollment should exist");
    let remote_control_target = stale_enrollment.remote_control_target.clone();
    let refreshed_enrollment = RemoteControlEnrollment {
        remote_control_target: remote_control_target.clone(),
        account_id: "account_id".to_string(),
        environment_id: "env_refreshed".to_string(),
        server_id: "srv_e_refreshed".to_string(),
        server_name: test_server_name(),
        remote_control_token: None,
        expires_at: None,
        next_refresh_at: None,
    };
    update_persisted_remote_control_enrollment(
        Some(state_db.as_ref()),
        &remote_control_target,
        "account_id",
        /*app_server_client_name*/ None,
        Some(&stale_enrollment),
        /*remote_control_enabled*/ Some(true),
    )
    .await
    .expect("stale enrollment should save");
    remote_handle
        .desired_state_tx
        .send_replace(RemoteControlDesiredState::Enabled {
            persistence_preference: Some(true),
        });
    let server_refreshed_enrollment = refreshed_enrollment.clone();
    let server_task = tokio::spawn(async move {
        let stale_pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            stale_pairing_request.headers.get("authorization"),
            Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
        );
        respond_with_status(stale_pairing_request.stream, "404 Not Found", "").await;

        let enroll_request = accept_http_request(&listener).await;
        assert_eq!(
            enroll_request.request_line,
            "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
        );
        respond_with_json(
            enroll_request.stream,
            remote_control_server_token_response(
                &server_refreshed_enrollment.server_id,
                &server_refreshed_enrollment.environment_id,
                TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
            ),
        )
        .await;

        let refreshed_pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            refreshed_pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            refreshed_pairing_request.headers.get("authorization"),
            Some(&format!(
                "Bearer {TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN}"
            ))
        );
        respond_with_json(
            refreshed_pairing_request.stream,
            pairing_response_json(
                &server_refreshed_enrollment.server_id,
                &server_refreshed_enrollment.environment_id,
            ),
        )
        .await;
    });
    let response = remote_handle
        .start_pairing(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
        .await
        .expect("pairing should re-enroll after stale enrollment");
    server_task.await.expect("server task should finish");

    assert_eq!(response, pairing_response("env_refreshed"));
    assert_eq!(
        state_db
            .get_remote_control_enrollment(
                &remote_control_target.websocket_url,
                "account_id",
                /*app_server_client_name*/ None,
            )
            .await
            .expect("refreshed enrollment should load")
            .expect("refreshed enrollment should exist")
            .remote_control_enabled,
        Some(true)
    );
}

#[tokio::test]
async fn remote_control_handle_discards_pairing_response_after_auth_change() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("account_id")),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("initial auth should save");
    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        /*auth_route_config*/ None,
    )
    .await;
    let remote_handle =
        remote_control_handle_with_current_enrollment(&remote_control_url, auth_manager.clone());
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(
                    RemoteControlPairingStartParams::default(),
                    /*app_server_client_name*/ None,
                )
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("next_account_id")),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .expect("next auth should save");
    auth_manager.reload().await;
    respond_with_json(
        pairing_request.stream,
        json!({
            "pairing_code": "stale-pairing-code",
            "manual_pairing_code": "ABCD-EFGH",
            "server_id": "srv_e_test",
            "environment_id": "env_test",
            "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await;

    assert_eq!(
        pairing_task
            .await
            .expect("pairing task should join")
            .expect_err("stale pairing response should be discarded")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );
}
