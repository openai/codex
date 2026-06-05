use super::super::protocol::StartRemoteControlPairingRequest;
use super::*;
use pretty_assertions::assert_eq;

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
            json!({
                "pairing_code": "pairing-code",
                "manual_pairing_code": "ABCD-EFGH",
                "server_id": "srv_e_test",
                "environment_id": "env_test",
                "expires_at": "3026-05-22T12:34:56Z",
            }),
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

    assert_eq!(
        response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
    );
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
            json!({
                "pairing_code": "pairing-code",
                "manual_pairing_code": "ABCD-EFGH",
                "server_id": "srv_e_test",
                "environment_id": "env_test",
                "expires_at": "3026-05-22T12:34:56Z",
            }),
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

    assert_eq!(
        response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
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
            json!({
                "pairing_code": "pairing-code",
                "manual_pairing_code": "ABCD-EFGH",
                "server_id": "srv_e_test",
                "environment_id": "env_test",
                "expires_at": "3026-05-22T12:34:56Z",
            }),
        )
        .await;
    });
    let codex_home = TempDir::new().expect("temp dir should create");
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
    )
    .expect("stale auth should save");
    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await;
    let mut fresh_auth = remote_control_auth_dot_json(Some("account_id"));
    fresh_auth
        .tokens
        .as_mut()
        .expect("fresh auth should include tokens")
        .access_token = "fresh-token".to_string();
    save_auth(
        codex_home.path(),
        &fresh_auth,
        AuthCredentialsStoreMode::File,
    )
    .expect("fresh auth should save");
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

    assert_eq!(
        response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
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

    remote_handle.disable();
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
    remote_handle.disable();
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
    };
    update_persisted_remote_control_enrollment(
        Some(state_db.as_ref()),
        &remote_control_target,
        "account_id",
        /*app_server_client_name*/ None,
        Some(&stale_enrollment),
    )
    .await
    .expect("stale enrollment should save");
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
            json!({
                "pairing_code": "pairing-code",
                "manual_pairing_code": "ABCD-EFGH",
                "server_id": server_refreshed_enrollment.server_id,
                "environment_id": server_refreshed_enrollment.environment_id,
                "expires_at": "3026-05-22T12:34:56Z",
            }),
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

    assert_eq!(
        response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_refreshed".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    assert_eq!(
        load_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &remote_control_target,
            "account_id",
            /*app_server_client_name*/ None,
        )
        .await
        .expect("refreshed enrollment should load"),
        Some(refreshed_enrollment)
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
    )
    .expect("initial auth should save");
    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
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
