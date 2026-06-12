use super::super::clients::list_remote_control_clients;
use super::super::clients::revoke_remote_control_client;
use super::*;
use codex_app_server_protocol::RemoteControlClient;
use codex_app_server_protocol::RemoteControlClientsListOrder;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use pretty_assertions::assert_eq;

fn client_management_handle(
    remote_control_url: String,
    auth_manager: Arc<AuthManager>,
) -> RemoteControlHandle {
    let desired_state_tx = watch::channel(RemoteControlDesiredState::Disabled).0;
    let (status_tx, _status_rx) = watch::channel(RemoteControlStatusChangedNotification {
        status: RemoteControlConnectionStatus::Disabled,
        server_name: test_server_name(),
        installation_id: TEST_INSTALLATION_ID.to_string(),
        environment_id: None,
    });
    RemoteControlHandle {
        desired_state_tx: Arc::new(desired_state_tx),
        desired_state_rpc_lock: Arc::new(Semaphore::new(1)),
        desired_state_persistence_lock: Arc::new(Semaphore::new(1)),
        status_tx: Arc::new(status_tx),
        state_db: None,
        remote_control_url,
        current_enrollment: Arc::new(RemoteControlEnrollmentState::new(/*enrollment*/ None)),
        pairing_persistence_key: watch::channel(None).0,
        pairing_persistence_key_required: false,
        auth_manager,
    }
}

fn empty_client_list() -> serde_json::Value {
    json!({
        "items": [],
        "cursor": null,
    })
}

#[tokio::test]
async fn remote_control_handle_lists_clients_while_disabled() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let request = accept_http_request(&listener).await;
        assert_eq!(
            request.request_line,
            "GET /backend-api/wham/remote/control/environments/env%20%2F%3F/clients?cursor=cursor+%2F%3F&limit=10&order=asc HTTP/1.1"
        );
        assert_eq!(
            request.headers.get("authorization"),
            Some(&"Bearer Access Token".to_string())
        );
        assert_eq!(
            request.headers.get(REMOTE_CONTROL_ACCOUNT_ID_HEADER),
            Some(&"account_id".to_string())
        );
        respond_with_json(
            request.stream,
            json!({
                "items": [{
                    "client_id": "client-123",
                    "account_user_id": "user-123",
                    "enrollment_status": "enrolled_device_key",
                    "display_name": "Anton Phone",
                    "device_type": "phone",
                    "platform": "ios",
                    "os_version": "19.0",
                    "device_model": "iPhone",
                    "app_version": "1.2.3",
                    "last_seen_at": "2026-03-05T07:00:00Z",
                    "last_seen_city": "San Francisco",
                }],
                "cursor": "next-cursor",
            }),
        )
        .await;
    });
    let handle = client_management_handle(remote_control_url, remote_control_auth_manager());

    let response = handle
        .list_clients(RemoteControlClientsListParams {
            environment_id: "env /?".to_string(),
            cursor: Some("cursor /?".to_string()),
            limit: Some(10),
            order: Some(RemoteControlClientsListOrder::Asc),
        })
        .await
        .expect("client list should succeed while remote control is disabled");
    server_task.await.expect("server task should finish");

    assert_eq!(
        response,
        RemoteControlClientsListResponse {
            data: vec![RemoteControlClient {
                client_id: "client-123".to_string(),
                display_name: Some("Anton Phone".to_string()),
                device_type: Some("phone".to_string()),
                platform: Some("ios".to_string()),
                os_version: Some("19.0".to_string()),
                device_model: Some("iPhone".to_string()),
                app_version: Some("1.2.3".to_string()),
                last_seen_at: Some(1_772_694_000),
            }],
            next_cursor: Some("next-cursor".to_string()),
        }
    );
}

#[tokio::test]
async fn remote_control_handle_revokes_client_while_disabled() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let request = accept_http_request(&listener).await;
        assert_eq!(
            request.request_line,
            "DELETE /backend-api/wham/remote/control/environments/env%20%2F%3F/clients/client%20%2F%3F HTTP/1.1"
        );
        respond_with_status(request.stream, "204 No Content", "").await;
    });
    let handle = client_management_handle(remote_control_url, remote_control_auth_manager());

    let response = handle
        .revoke_client(RemoteControlClientsRevokeParams {
            environment_id: "env /?".to_string(),
            client_id: "client /?".to_string(),
        })
        .await
        .expect("client revoke should succeed while remote control is disabled");
    server_task.await.expect("server task should finish");

    assert_eq!(response, RemoteControlClientsRevokeResponse {});
}

#[tokio::test]
async fn list_remote_control_clients_recovers_auth_after_unauthorized() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_request.headers.get("authorization"),
            Some(&"Bearer stale-token".to_string())
        );
        respond_with_status(stale_request.stream, "401 Unauthorized", "").await;

        let recovered_request = accept_http_request(&listener).await;
        assert_eq!(
            recovered_request.headers.get("authorization"),
            Some(&"Bearer fresh-token".to_string())
        );
        respond_with_json(recovered_request.stream, empty_client_list()).await;
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

    let response = list_remote_control_clients(
        &remote_control_url,
        &auth_manager,
        RemoteControlClientsListParams {
            environment_id: "env-123".to_string(),
            ..Default::default()
        },
    )
    .await
    .expect("client list should recover auth");
    server_task.await.expect("server task should finish");

    assert_eq!(
        response,
        RemoteControlClientsListResponse {
            data: Vec::new(),
            next_cursor: None,
        }
    );
}

#[tokio::test]
async fn list_remote_control_clients_retries_unauthorized_only_once() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let stale_request = accept_http_request(&listener).await;
        assert_eq!(
            stale_request.headers.get("authorization"),
            Some(&"Bearer stale-token".to_string())
        );
        respond_with_status(stale_request.stream, "401 Unauthorized", "").await;

        let recovered_request = accept_http_request(&listener).await;
        assert_eq!(
            recovered_request.headers.get("authorization"),
            Some(&"Bearer fresh-token".to_string())
        );
        respond_with_status(recovered_request.stream, "401 Unauthorized", "").await;

        assert!(
            timeout(Duration::from_millis(100), accept_http_request(&listener))
                .await
                .is_err()
        );
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

    let err = list_remote_control_clients(
        &remote_control_url,
        &auth_manager,
        RemoteControlClientsListParams {
            environment_id: "env-123".to_string(),
            ..Default::default()
        },
    )
    .await
    .expect_err("second unauthorized response should fail");
    server_task.await.expect("server task should finish");

    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn revoke_remote_control_client_does_not_retry_forbidden() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let request = accept_http_request(&listener).await;
        respond_with_status_and_headers(
            request.stream,
            "403 Forbidden",
            &[("x-request-id", "request-123"), ("cf-ray", "ray-123")],
            "forbidden",
        )
        .await;
    });

    let err = revoke_remote_control_client(
        &remote_control_url,
        &remote_control_auth_manager(),
        RemoteControlClientsRevokeParams {
            environment_id: "env-123".to_string(),
            client_id: "client-123".to_string(),
        },
    )
    .await
    .expect_err("forbidden revoke should fail");
    server_task.await.expect("server task should finish");

    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        err.to_string(),
        format!(
            "remote control client revoke failed at `{remote_control_url}wham/remote/control/environments/env-123/clients/client-123`: HTTP 403 Forbidden, request-id: request-123, cf-ray: ray-123, body: forbidden"
        )
    );
}

#[tokio::test]
async fn list_remote_control_clients_preserves_decode_error_context() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let server_task = tokio::spawn(async move {
        let request = accept_http_request(&listener).await;
        respond_with_status(request.stream, "200 OK", "{").await;
    });

    let err = list_remote_control_clients(
        &remote_control_url,
        &remote_control_auth_manager(),
        RemoteControlClientsListParams {
            environment_id: "env-123".to_string(),
            ..Default::default()
        },
    )
    .await
    .expect_err("malformed client list should fail");
    server_task.await.expect("server task should finish");

    assert!(
        err.to_string().contains(
            "failed to parse remote control client list response from `http://127.0.0.1:"
        )
    );
    assert!(err.to_string().contains("HTTP 200 OK"));
    assert!(err.to_string().contains("body: {"));
    assert!(err.to_string().contains("decode error:"));
}
