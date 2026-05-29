use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn remote_control_handle_disable_clears_stale_pairing_client() {
    let remote_handle = remote_control_handle_with_pairing_client(
        TEST_REMOTE_CONTROL_URL,
        watch::channel(/*init*/ 0u64).1,
    );

    assert_eq!(
        remote_handle.disable(),
        RemoteControlStatusChangedNotification {
            status: RemoteControlConnectionStatus::Disabled,
            server_name: test_server_name(),
            installation_id: TEST_INSTALLATION_ID.to_string(),
            environment_id: None,
        }
    );
    remote_handle.enable().expect("enable should succeed");
    assert_eq!(
        remote_handle
            .start_pairing(RemoteControlPairingStartParams { manual_code: false })
            .await
            .expect_err("re-enabled remote control should wait for refreshed pairing auth")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );
}

#[tokio::test]
async fn remote_control_disable_during_websocket_handshake_clears_pairing_client() {
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
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;

    let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("remote control should connect in time")
        .expect("listener accept should succeed");
    let remote_handle_for_callback = remote_handle.clone();
    let mut websocket = accept_hdr_async(
        stream,
        move |_request: &tungstenite::handshake::server::Request,
              response: tungstenite::handshake::server::Response| {
            remote_handle_for_callback.disable();
            Ok(response)
        },
    )
    .await
    .expect("websocket handshake should succeed");
    expect_remote_control_connection_closed(
        &mut websocket,
        "disabled remote control should close the websocket handshake",
    )
    .await;
    assert!(
        remote_handle
            .pairing
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none(),
        "disabling during websocket handshake should clear pairing auth"
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_handle_rejects_pairing_client_after_auth_change() {
    let (auth_change_tx, auth_change_rx) = watch::channel(/*init*/ 0u64);
    let remote_handle =
        remote_control_handle_with_pairing_client(TEST_REMOTE_CONTROL_URL, auth_change_rx);
    auth_change_tx.send_modify(|revision| *revision += 1);

    assert_eq!(
        remote_handle
            .start_pairing(RemoteControlPairingStartParams::default())
            .await
            .expect_err("pairing should wait for current-account enrollment")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
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
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        auth_manager.auth_change_receiver(),
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    assert_eq!(
        pairing_request.request_line,
        "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
    );
    assert_eq!(
        pairing_request.headers.get("authorization"),
        Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&pairing_request.body)
            .expect("pairing request body should deserialize"),
        json!({ "manual_code": false })
    );

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

#[tokio::test]
async fn remote_control_handle_discards_pairing_response_after_disable() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        watch::channel(/*init*/ 0u64).1,
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    remote_handle.disable();
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
            .expect_err("disabled remote control should discard pairing response")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );
}

#[tokio::test]
async fn remote_control_handle_keeps_pairing_response_after_pairing_auth_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        watch::channel(/*init*/ 0u64).1,
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    let generation = remote_handle
        .pairing
        .generation
        .load(std::sync::atomic::Ordering::Relaxed);
    *remote_handle
        .pairing
        .client
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(RemoteControlPairingClient::new(
            &normalize_remote_control_url(&remote_control_url)
                .expect("remote control url should normalize"),
            TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN.to_string(),
            "srv_e_test".to_string(),
            "env_test".to_string(),
            OffsetDateTime::parse(
                TEST_REMOTE_CONTROL_SERVER_TOKEN_EXPIRES_AT,
                &time::format_description::well_known::Rfc3339,
            )
            .expect("server token expiry should parse"),
            /*auth_change_revision*/ 0,
            generation,
        ));
    respond_with_json(
        pairing_request.stream,
        json!({
            "pairing_code": "fresh-pairing-code",
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
            .expect("pairing response should be kept")
            .pairing_code,
        "fresh-pairing-code"
    );
}

#[tokio::test]
async fn remote_control_handle_keeps_pairing_response_after_connection_cycle_end() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        watch::channel(/*init*/ 0u64).1,
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    clear_pairing_client(&remote_handle.pairing);
    respond_with_json(
        pairing_request.stream,
        json!({
            "pairing_code": "fresh-pairing-code",
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
            .expect("pairing response should be kept")
            .pairing_code,
        "fresh-pairing-code"
    );
}

#[tokio::test]
async fn remote_control_handle_discards_pairing_response_after_disable_and_reenable() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        watch::channel(/*init*/ 0u64).1,
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    remote_handle.disable();
    remote_handle.enable().expect("enable should succeed");
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
            .expect_err("disabled remote control should discard pairing response")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );
}

#[tokio::test]
async fn remote_control_handle_keeps_refreshed_pairing_auth_after_stale_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_handle = remote_control_handle_with_pairing_client(
        &remote_control_url,
        watch::channel(/*init*/ 0u64).1,
    );
    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });

    let pairing_request = accept_http_request(&listener).await;
    let generation = remote_handle
        .pairing
        .generation
        .load(std::sync::atomic::Ordering::Relaxed);
    let refreshed_pairing_client = RemoteControlPairingClient::new(
        &normalize_remote_control_url(&remote_control_url)
            .expect("remote control url should normalize"),
        TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN.to_string(),
        "srv_e_test".to_string(),
        "env_test".to_string(),
        OffsetDateTime::parse(
            TEST_REMOTE_CONTROL_SERVER_TOKEN_EXPIRES_AT,
            &time::format_description::well_known::Rfc3339,
        )
        .expect("server token expiry should parse"),
        /*auth_change_revision*/ 0,
        generation,
    );
    *remote_handle
        .pairing
        .client
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(refreshed_pairing_client.clone());
    respond_with_status(pairing_request.stream, "401 Unauthorized", "stale token").await;

    assert_eq!(
        pairing_task
            .await
            .expect("pairing task should join")
            .expect_err("stale pairing token should be rejected")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(
        remote_handle
            .pairing
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|pairing_client| {
                pairing_client.matches_pairing_auth(&refreshed_pairing_client)
            }),
        "stale pairing rejection should keep refreshed pairing auth"
    );
    assert_eq!(*remote_handle.pairing_refresh_tx.borrow(), 0);
}

#[tokio::test]
async fn remote_control_handle_clears_pairing_client_after_auth_change() {
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
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, remote_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url,
            installation_id: TEST_INSTALLATION_ID.to_string(),
        },
        Some(remote_control_state_runtime(&codex_home).await),
        auth_manager.clone(),
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
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("next_account_id")),
        AuthCredentialsStoreMode::File,
    )
    .expect("next auth should save");
    auth_manager.reload().await;
    expect_remote_control_connection_closed(
        &mut first_websocket,
        "auth change should close the stale websocket",
    )
    .await;
    assert_eq!(
        remote_handle
            .start_pairing(RemoteControlPairingStartParams::default())
            .await
            .expect_err("pairing should wait for current-account enrollment")
            .to_string(),
        "remote control pairing is unavailable until enrollment completes"
    );

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_next",
            "env_next",
            TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut second_websocket = accept_remote_control_connection(&listener).await;
    second_websocket
        .close(None)
        .await
        .expect("second websocket should close");

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_refreshes_server_token_while_connected() {
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
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    remote_handle.request_pairing_auth_refresh();
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
    assert!(
        timeout(Duration::from_millis(100), first_websocket.next())
            .await
            .is_err(),
        "server token refresh should keep the websocket open"
    );

    let pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
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
    assert_eq!(
        pairing_task
            .await
            .expect("pairing task should join")
            .expect("pairing should use refreshed server token"),
        codex_app_server_protocol::RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "env_test".to_string(),
            expires_at: 33_336_362_096,
        }
    );

    first_websocket
        .close(None)
        .await
        .expect("first websocket should close");

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_schedules_server_token_refresh_while_connected() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
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

    let scheduled_refresh_expires_at = (OffsetDateTime::now_utc() + time::Duration::seconds(35))
        .format(&time::format_description::well_known::Rfc3339)
        .expect("scheduled refresh expiry should format");
    let enroll_request = accept_http_request(&listener).await;
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response_with_expires_at(
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
            &scheduled_refresh_expires_at,
        ),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    let refresh_request = timeout(Duration::from_secs(10), accept_http_request(&listener))
        .await
        .expect("scheduled server token refresh should arrive");
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
    assert!(
        timeout(Duration::from_millis(100), first_websocket.next())
            .await
            .is_err(),
        "scheduled server token refresh should keep the websocket open"
    );

    first_websocket
        .close(None)
        .await
        .expect("first websocket should close");
    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_connected_refresh_rejection_clears_pairing_auth() {
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
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    remote_handle.request_pairing_auth_refresh();
    let refresh_request = accept_http_request(&listener).await;
    assert_eq!(
        refresh_request.request_line,
        "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
    );
    respond_with_status(refresh_request.stream, "401 Unauthorized", "stale token").await;
    expect_remote_control_connection_closed(
        &mut first_websocket,
        "rejected refresh should close the websocket",
    )
    .await;
    assert!(
        remote_handle
            .pairing
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none(),
        "rejected refresh should clear cached pairing auth"
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_auth_change_cancels_connected_refresh() {
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
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, remote_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url,
            installation_id: TEST_INSTALLATION_ID.to_string(),
        },
        Some(remote_control_state_runtime(&codex_home).await),
        auth_manager.clone(),
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
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    remote_handle.request_pairing_auth_refresh();
    let stalled_refresh_request = accept_http_request(&listener).await;
    assert_eq!(
        stalled_refresh_request.request_line,
        "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
    );

    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("next_account_id")),
        AuthCredentialsStoreMode::File,
    )
    .expect("next auth should save");
    auth_manager.reload().await;
    expect_remote_control_connection_closed(
        &mut first_websocket,
        "auth change should close websocket while refresh is stalled",
    )
    .await;
    drop(stalled_refresh_request);

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_next",
            "env_next",
            TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut second_websocket = accept_remote_control_connection(&listener).await;
    second_websocket
        .close(None)
        .await
        .expect("second websocket should close");

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_connected_refresh_404_reenrolls() {
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
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    remote_handle.request_pairing_auth_refresh();
    let refresh_request = accept_http_request(&listener).await;
    assert_eq!(
        refresh_request.request_line,
        "POST /backend-api/wham/remote/control/server/refresh HTTP/1.1"
    );
    respond_with_status(refresh_request.stream, "404 Not Found", "").await;
    expect_remote_control_connection_closed(
        &mut first_websocket,
        "stale enrollment refresh should close the websocket",
    )
    .await;

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        remote_control_server_token_response(
            "srv_e_next",
            "env_next",
            TEST_REFRESHED_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut second_websocket = accept_remote_control_connection(&listener).await;
    second_websocket
        .close(None)
        .await
        .expect("second websocket should close");

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_pairing_rejection_refreshes_server_token_while_connected() {
    remote_control_pairing_rejection_recovers_while_connected("401 Unauthorized").await;
}

#[tokio::test]
async fn remote_control_pairing_not_found_refreshes_server_token_while_connected() {
    remote_control_pairing_rejection_recovers_while_connected("404 Not Found").await;
}

async fn remote_control_pairing_rejection_recovers_while_connected(pair_status: &str) {
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
            "srv_e_test",
            "env_test",
            TEST_REMOTE_CONTROL_SERVER_TOKEN,
        ),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;
    timeout(Duration::from_secs(5), async {
        while remote_handle.status().status != RemoteControlConnectionStatus::Connected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("remote control should publish connected before pairing");

    let stale_pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
    let pairing_request = accept_http_request(&listener).await;
    assert_eq!(
        pairing_request.headers.get("authorization"),
        Some(&format!("Bearer {TEST_REMOTE_CONTROL_SERVER_TOKEN}"))
    );
    respond_with_status(pairing_request.stream, pair_status, "stale token").await;
    assert_eq!(
        stale_pairing_task
            .await
            .expect("stale pairing task should join")
            .expect_err("stale pairing token should be rejected")
            .kind(),
        if pair_status == "404 Not Found" {
            std::io::ErrorKind::NotFound
        } else {
            std::io::ErrorKind::PermissionDenied
        }
    );
    assert!(
        remote_handle
            .pairing
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none(),
        "pairing rejection should clear cached pairing auth"
    );
    assert_eq!(*remote_handle.pairing_refresh_tx.borrow(), 1);

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
    assert!(
        timeout(Duration::from_millis(100), first_websocket.next())
            .await
            .is_err(),
        "pairing auth refresh should keep the websocket open"
    );

    let refreshed_pairing_task = tokio::spawn({
        let remote_handle = remote_handle.clone();
        async move {
            remote_handle
                .start_pairing(RemoteControlPairingStartParams::default())
                .await
        }
    });
    let pairing_request = accept_http_request(&listener).await;
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
    refreshed_pairing_task
        .await
        .expect("refreshed pairing task should join")
        .expect("refreshed pairing token should pair");

    first_websocket
        .close(None)
        .await
        .expect("websocket should close");
    shutdown_token.cancel();
    let _ = remote_task.await;
}
