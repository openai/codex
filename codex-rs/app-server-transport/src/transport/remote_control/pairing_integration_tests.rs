use super::super::protocol::RemoteControlTarget;
use super::super::protocol::StartRemoteControlPairingRequest;
use super::super::protocol::normalize_remote_control_url;
use super::super::server::RemoteControlServer;
use super::super::server::SharedRemoteControlServer;
use super::*;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use pretty_assertions::assert_eq;
use time::OffsetDateTime;

#[tokio::test]
async fn remote_control_pairing_posts_server_token_and_maps_response() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_target = remote_control_target_for_listener(&listener);
    assert_eq!(
        remote_control_target.pair_url,
        format!(
            "http://{}/backend-api/wham/remote/control/server/pair",
            listener.local_addr().expect("listener should have addr")
        )
    );
    let pairing_server = tokio::spawn(async move {
        let pairing_request = accept_http_request(&listener).await;
        assert_eq!(
            pairing_request.request_line,
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );
        assert_eq!(
            pairing_request.headers.get("authorization"),
            Some(&"Bearer remote-control-token".to_string())
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&pairing_request.body)
                .expect("pairing body should deserialize"),
            json!({ "manual_code": true })
        );
        respond_with_json(
            pairing_request.stream,
            json!({
                "pairing_code": "pairing-code",
                "manual_pairing_code": "ABCD-EFGH",
                "server_id": "server-id",
                "environment_id": "environment-id",
                "expires_at": "3026-05-22T12:34:56Z",
            }),
        )
        .await
    });
    assert_eq!(
        remote_control_server_state(
            OffsetDateTime::from_unix_timestamp(33_336_362_096)
                .expect("future timestamp should parse"),
        )
        .start_pairing(
            &remote_control_target,
            StartRemoteControlPairingRequest { manual_code: true },
        )
        .await
        .expect("pairing should succeed"),
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "environment-id".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    pairing_server.await.expect("pairing server should join");
}

#[tokio::test]
async fn remote_control_pairing_preserves_backend_error_context() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_target = remote_control_target_for_listener(&listener);
    let expected_pair_url = remote_control_target.pair_url.clone();
    let pairing_server = tokio::spawn(async move {
        let pairing_request = accept_http_request(&listener).await;
        respond_with_status_and_headers(
            pairing_request.stream,
            "503 Service Unavailable",
            &[("x-request-id", "request-123"), ("cf-ray", "ray-123")],
            "pairing unavailable",
        )
        .await
    });

    assert_eq!(
        remote_control_server_state(
            OffsetDateTime::from_unix_timestamp(33_336_362_096)
                .expect("future timestamp should parse"),
        )
        .start_pairing(
            &remote_control_target,
            StartRemoteControlPairingRequest { manual_code: false },
        )
        .await
        .expect_err("pairing should fail")
        .to_string(),
        format!(
            "remote control pairing failed at `{expected_pair_url}`: HTTP 503 Service Unavailable, request-id: request-123, cf-ray: ray-123, body: pairing unavailable"
        )
    );
    pairing_server.await.expect("pairing server should join");
}

#[tokio::test]
async fn remote_control_pairing_rejects_expired_server_token() {
    let err = remote_control_server_state(
        OffsetDateTime::from_unix_timestamp(0).expect("expired timestamp should parse"),
    )
    .start_pairing(
        &RemoteControlTarget {
            websocket_url: "ws://unused".to_string(),
            enroll_url: "http://unused".to_string(),
            refresh_url: "http://unused".to_string(),
            pair_url: "http://unused".to_string(),
        },
        StartRemoteControlPairingRequest { manual_code: false },
    )
    .await
    .expect_err("expired server token should fail pairing");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        err.to_string(),
        "remote control pairing is unavailable because the server token expired"
    );
}

#[tokio::test]
async fn remote_control_pairing_refreshes_server_token_before_posting() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let codex_home = TempDir::new().expect("temp dir should create");
    let server = shared_remote_control_server_for_listener(
        &listener,
        &codex_home,
        remote_control_auth_manager(),
    )
    .await;
    server
        .replace(Some(RemoteControlServer {
            account_id: "account_id".to_string(),
            environment_id: "env_test".to_string(),
            server_id: "srv_e_test".to_string(),
            server_name: "server-name".to_string(),
            remote_control_token: None,
            expires_at: None,
        }))
        .await;

    let pairing_task = tokio::spawn({
        let server = server.clone();
        async move {
            server
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
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
            "manual_pairing_code": null,
            "server_id": "srv_e_test",
            "environment_id": "env_test",
            "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await;
    pairing_task
        .await
        .expect("pairing task should join")
        .expect("pairing should succeed");
}

#[tokio::test]
async fn remote_control_pairing_does_not_require_websocket_connection() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, remote_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url,
            installation_id: TEST_INSTALLATION_ID.to_string(),
        },
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let enroll_request = accept_http_request(&listener).await;
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_initial",
            "env_initial",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let stalled_websocket_request = accept_http_request(&listener).await;
    assert_eq!(
        stalled_websocket_request.request_line,
        "GET /backend-api/wham/remote/control/server HTTP/1.1"
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
    let stalled_pairing_request = accept_http_request(&listener).await;
    assert_eq!(
        stalled_pairing_request.request_line,
        "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
    );
    assert_eq!(
        remote_handle.status().status,
        RemoteControlConnectionStatus::Connecting
    );
    respond_with_json(
        stalled_pairing_request.stream,
        json!({
            "pairing_code": "pairing-code",
            "manual_pairing_code": null,
            "server_id": "srv_e_initial",
            "environment_id": "env_initial",
            "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await;
    pairing_task
        .await
        .expect("pairing task should join")
        .expect("pairing should succeed before websocket connects");

    drop(stalled_websocket_request);
    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_auth_change_cancels_in_flight_pairing() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
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
    let server =
        shared_remote_control_server_for_listener(&listener, &codex_home, auth_manager.clone())
            .await;
    let pairing_task = tokio::spawn({
        let server = server.clone();
        async move {
            server
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
    let enroll_request = accept_http_request(&listener).await;
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_initial",
            "env_initial",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let stalled_pairing_request = accept_http_request(&listener).await;
    assert_eq!(
        stalled_pairing_request.request_line,
        "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
    );
    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("next_account_id")),
        AuthCredentialsStoreMode::File,
    )
    .expect("next auth should save");
    auth_manager.reload().await;
    respond_with_json(
        stalled_pairing_request.stream,
        json!({
            "pairing_code": "stale-pairing-code",
            "manual_pairing_code": null,
            "server_id": "srv_e_initial",
            "environment_id": "env_initial",
            "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await;
    assert_eq!(
        pairing_task
            .await
            .expect("pairing task should join")
            .expect_err("stale pairing should be discarded")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );
}

fn remote_control_target_for_listener(listener: &TcpListener) -> RemoteControlTarget {
    normalize_remote_control_url(&remote_control_url_for_listener(listener))
        .expect("target should parse")
}

fn remote_control_server_state(expires_at: OffsetDateTime) -> RemoteControlServer {
    RemoteControlServer {
        account_id: "account-id".to_string(),
        environment_id: "environment-id".to_string(),
        server_id: "server-id".to_string(),
        server_name: "server-name".to_string(),
        remote_control_token: Some("remote-control-token".to_string()),
        expires_at: Some(expires_at),
    }
}

async fn shared_remote_control_server_for_listener(
    listener: &TcpListener,
    codex_home: &TempDir,
    auth_manager: Arc<AuthManager>,
) -> SharedRemoteControlServer {
    SharedRemoteControlServer::new(
        remote_control_url_for_listener(listener),
        TEST_INSTALLATION_ID.to_string(),
        "server-name".to_string(),
        Some(remote_control_state_runtime(codex_home).await),
        auth_manager,
    )
}
