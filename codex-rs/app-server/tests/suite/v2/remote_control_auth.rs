//! Exercises login lifetime boundaries through public RPCs and a real relay websocket.

use super::*;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::encode_id_token;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::ServerRequest;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio_tungstenite::WebSocketStream;

struct RelayClient {
    socket: WebSocketStream<TcpStream>,
    next_seq: u64,
}

impl RelayClient {
    async fn send(&mut self, message: Value) -> Result<()> {
        self.socket
            .send(Message::Text(
                json!({
                    "type": "client_message", "client_id": "client-a", "stream_id": "stream-a",
                    "seq_id": self.next_seq, "cursor": format!("cursor-{}", self.next_seq),
                    "message": message,
                })
                .to_string()
                .into(),
            ))
            .await?;
        self.next_seq += 1;
        Ok(())
    }

    async fn response(&mut self, id: i64) -> Result<Value> {
        timeout(DEFAULT_TIMEOUT, async {
            loop {
                let message = self.socket.next().await.context("relay disconnected")??;
                if let Message::Text(text) = message {
                    let envelope: Value = serde_json::from_str(&text)?;
                    if envelope["type"] == "server_message" && envelope["message"]["id"] == id {
                        return Ok(envelope["message"].clone());
                    }
                }
            }
        })
        .await?
    }

    async fn closed(&mut self) -> Result<()> {
        timeout(DEFAULT_TIMEOUT, async {
            loop {
                match self.socket.next().await {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                    Some(Ok(_)) => {}
                }
            }
        })
        .await?;
        Ok(())
    }

    async fn assert_account(&mut self, app: &mut TestAppServer) -> Result<()> {
        let local = app.send_request("account/read", Some(json!({}))).await?;
        let expected: Value = timeout(DEFAULT_TIMEOUT, app.read_response(local)).await??;
        self.send(json!({"id": 2, "method": "account/read", "params": {}}))
            .await?;
        assert_eq!(
            self.response(/*id*/ 2).await?.get("result"),
            Some(&expected)
        );
        Ok(())
    }
}

fn external_token(user: &str, account: &str, revision: &str) -> Result<String> {
    encode_id_token(
        &ChatGptIdTokenClaims::new()
            .chatgpt_account_id(account)
            .chatgpt_user_id(user)
            .plan_type("pro")
            .email(format!("{revision}@example.com")),
    )
}

async fn login(app: &mut TestAppServer, user: &str, account: &str, revision: &str) -> Result<()> {
    let id = app
        .send_chatgpt_auth_tokens_login_request(
            external_token(user, account, revision)?,
            account.to_string(),
            Some("pro".to_string()),
        )
        .await?;
    wait_for_response(app, id).await
}

async fn open_relay(app: &mut TestAppServer, listener: &TcpListener) -> Result<RelayClient> {
    let id = app.send_remote_control_ephemeral_enable_request().await?;
    wait_for_response(app, id).await?;
    let request = timeout(DEFAULT_TIMEOUT, read_http_request(listener)).await??;
    assert!(
        request.request_line.contains("/server/enroll ")
            || request.request_line.contains("/server/refresh "),
        "{}",
        request.request_line
    );
    respond_with_json(
        request.reader.into_inner(),
        json!({
            "server_id": "server-id", "environment_id": "environment-id",
            "remote_control_token": "remote-control-token", "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await?;
    let (stream, _) = timeout(DEFAULT_TIMEOUT, listener.accept()).await??;
    let socket = timeout(
        DEFAULT_TIMEOUT,
        tokio_tungstenite::accept_hdr_async(
            stream,
            |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                assert!(!request.headers().contains_key("x-codex-subscribe-cursor"));
                Ok(response)
            },
        ),
    )
    .await??;
    let mut relay = RelayClient {
        socket,
        next_seq: 0,
    };
    // A replacement must not retain the old logical client or replay its unacknowledged results.
    relay
        .socket
        .send(Message::Text(
            json!({
                "type": "ping", "client_id": "client-a", "stream_id": "stream-a",
            })
            .to_string()
            .into(),
        ))
        .await?;
    timeout(DEFAULT_TIMEOUT, async {
        loop {
            let message = relay
                .socket
                .next()
                .await
                .context("relay disconnected before pong")??;
            if let Message::Text(text) = message {
                let envelope: Value = serde_json::from_str(&text)?;
                assert_ne!(envelope["type"], "server_message");
                if envelope["type"] == "pong" {
                    assert_eq!(envelope["status"], "unknown");
                    return Ok::<(), anyhow::Error>(());
                }
            }
        }
    })
    .await??;
    relay
        .send(json!({"id": 1, "method": "initialize", "params": {
            "clientInfo": {"name": "remote-auth-test", "version": "0.1.0"},
            "capabilities": {"experimentalApi": true},
        }}))
        .await?;
    assert!(relay.response(/*id*/ 1).await?.get("result").is_some());
    relay.send(json!({"method": "initialized"})).await?;
    Ok(relay)
}

#[tokio::test]
async fn same_owner_refresh_preserves_live_relay() -> Result<()> {
    let home = TempDir::new()?;
    let listener = configured_remote_control_listener(home.path()).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    login(&mut app, "user-a", "account-a", "initial").await?;
    let mut relay = open_relay(&mut app, &listener).await?;
    login(&mut app, "user-a", "account-a", "refreshed").await?;
    relay.assert_account(&mut app).await
}

#[tokio::test]
async fn owner_change_retires_live_relay() -> Result<()> {
    for (user, account) in [("user-b", "account-a"), ("user-a", "account-b")] {
        let home = TempDir::new()?;
        let listener = configured_remote_control_listener(home.path()).await?;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .without_auto_env()
            .build_initialized()
            .await?;
        login(&mut app, "user-a", "account-a", "initial").await?;
        let mut old = open_relay(&mut app, &listener).await?;
        old.assert_account(&mut app).await?;
        login(&mut app, user, account, "replacement").await?;
        old.closed().await?;
        let id = app.send_remote_control_status_read_request().await?;
        let status: RemoteControlStatusReadResponse =
            timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
        assert_eq!(status.status, RemoteControlConnectionStatus::Disabled);
        let mut fresh = open_relay(&mut app, &listener).await?;
        fresh.assert_account(&mut app).await?;
    }
    Ok(())
}

#[tokio::test]
async fn owner_change_discards_pending_and_queued_pairing() -> Result<()> {
    let home = TempDir::new()?;
    let listener = configured_remote_control_listener(home.path()).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    login(&mut app, "user-a", "account-a", "initial").await?;
    let mut old = open_relay(&mut app, &listener).await?;
    let pending = app
        .send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: false,
        })
        .await?;
    let held = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
    assert!(held.request_line.contains("/server/pair "));
    old.send(
        json!({"id": 3, "method": "remoteControl/pairing/start", "params": {"manualCode": false}}),
    )
    .await?;
    // account/read uses another queue. Its response proves the prior pairing RPC was received.
    old.assert_account(&mut app).await?;
    login(&mut app, "user-b", "account-a", "replacement").await?;
    let mut fresh = open_relay(&mut app, &listener).await?;
    let sentinel = app
        .send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: true,
        })
        .await?;
    // The old socket can already be cancelled, so delivery of this obsolete response is optional.
    let _ = respond_with_json(held.reader.into_inner(), json!({
        "pairing_code": "obsolete", "server_id": "server-id", "environment_id": "environment-id",
        "expires_at": "3026-05-22T12:34:56Z",
    })).await;
    let next = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
    assert!(next.request_line.contains("/server/pair "));
    assert_eq!(
        serde_json::from_str::<Value>(&next.body)?,
        json!({"manual_code": true})
    );
    respond_with_json(
        next.reader.into_inner(),
        json!({
            "pairing_code": "current", "server_id": "server-id", "manual_pairing_code": "ABCD-EFGH",
            "environment_id": "environment-id", "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await?;
    timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(pending)),
    )
    .await??;
    wait_for_response(&mut app, sentinel).await?;
    old.closed().await?;
    fresh.assert_account(&mut app).await
}

#[tokio::test]
async fn client_revoke_retry_stays_with_original_owner() -> Result<()> {
    for refreshed_user in ["user-a", "user-b"] {
        let home = TempDir::new()?;
        let listener = configured_remote_control_listener(home.path()).await?;
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .without_auto_env()
            .build_initialized()
            .await?;
        login(&mut app, "user-a", "account-a", "initial").await?;
        let old = app
            .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
                environment_id: "environment-id".to_string(),
                client_id: "old-client".to_string(),
            })
            .await?;
        let first = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
        assert_eq!(
            first.request_line,
            "DELETE /backend-api/wham/remote/control/environments/environment-id/clients/old-client HTTP/1.1"
        );
        respond_with_status(first.reader.into_inner(), "401 Unauthorized", "expired").await?;
        let request = timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
        let ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } = request else {
            anyhow::bail!("expected external authentication refresh");
        };
        app.send_response(
            request_id,
            serde_json::to_value(ChatgptAuthTokensRefreshResponse {
                access_token: external_token(refreshed_user, "account-a", "refreshed")?,
                chatgpt_account_id: "account-a".to_string(),
                chatgpt_plan_type: Some("pro".to_string()),
            })?,
        )
        .await?;
        let (next_id, client) = if refreshed_user == "user-a" {
            (old, "old-client")
        } else {
            timeout(
                DEFAULT_TIMEOUT,
                app.read_stream_until_error_message(RequestId::Integer(old)),
            )
            .await??;
            login(&mut app, refreshed_user, "account-a", "refreshed").await?;
            let id = app
                .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
                    environment_id: "environment-id".to_string(),
                    client_id: "new-client".to_string(),
                })
                .await?;
            (id, "new-client")
        };
        let next = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
        assert_eq!(
            next.request_line,
            format!(
                "DELETE /backend-api/wham/remote/control/environments/environment-id/clients/{client} HTTP/1.1"
            )
        );
        respond_with_json(next.reader.into_inner(), json!({})).await?;
        let result: RemoteControlClientsRevokeResponse =
            timeout(DEFAULT_TIMEOUT, app.read_response(next_id)).await??;
        assert_eq!(result, RemoteControlClientsRevokeResponse {});
    }
    Ok(())
}

#[tokio::test]
async fn owner_change_rejects_queued_client_revoke() -> Result<()> {
    let home = TempDir::new()?;
    let listener = configured_remote_control_listener(home.path()).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    login(&mut app, "user-a", "account-a", "initial").await?;
    let mut relay = open_relay(&mut app, &listener).await?;
    let pending = app
        .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
            environment_id: "environment-id".to_string(),
            client_id: "pending".to_string(),
        })
        .await?;
    let held = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
    relay
        .send(
            json!({"id": 3, "method": "remoteControl/client/revoke", "params": {
                "environmentId": "environment-id", "clientId": "obsolete",
            }}),
        )
        .await?;
    relay.assert_account(&mut app).await?;
    login(&mut app, "user-b", "account-a", "replacement").await?;
    let sentinel = app
        .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
            environment_id: "environment-id".to_string(),
            client_id: "current".to_string(),
        })
        .await?;
    let _ = respond_with_json(held.reader.into_inner(), json!({})).await;
    // Client management works even while disabled, so disablement cannot mask a missing gate.
    let next = timeout(DEFAULT_TIMEOUT, read_http_request(&listener)).await??;
    assert_eq!(
        next.request_line,
        "DELETE /backend-api/wham/remote/control/environments/environment-id/clients/current HTTP/1.1"
    );
    respond_with_json(next.reader.into_inner(), json!({})).await?;
    timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(pending)),
    )
    .await??;
    wait_for_response(&mut app, sentinel).await?;
    relay.closed().await
}

#[tokio::test]
async fn logout_login_enable_recovers_first_unauthorized_enrollment() -> Result<()> {
    let home = TempDir::new()?;
    let listener = configured_remote_control_listener(home.path()).await?;
    let oauth = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::path("/oauth/revoke"))
        .respond_with(wiremock::ResponseTemplate::new(/*s*/ 200))
        .expect(1)
        .mount(&oauth)
        .await;
    let revoke_url = format!("{}/oauth/revoke", oauth.uri());
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("CODEX_REVOKE_TOKEN_URL_OVERRIDE", Some(revoke_url.as_str()))])
        .without_auto_env()
        .build_initialized()
        .await?;
    let enabled = app.send_remote_control_ephemeral_enable_request().await?;
    wait_for_response(&mut app, enabled).await?;
    let (_, reader) = timeout(DEFAULT_TIMEOUT, read_enroll_request(&listener)).await??;
    respond_with_json(
        reader.into_inner(),
        serde_json::json!({
            "server_id": "server-a", "environment_id": "environment-a",
            "remote_control_token": "token-a", "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await?;
    let (stream, _) = timeout(DEFAULT_TIMEOUT, listener.accept()).await??;
    let mut websocket = timeout(DEFAULT_TIMEOUT, accept_async(stream)).await??;
    let logout = app.send_logout_account_request().await?;
    wait_for_response(&mut app, logout).await?;
    timeout(DEFAULT_TIMEOUT, async {
        loop {
            let notification = app
                .read_stream_until_notification_message("remoteControl/status/changed")
                .await?;
            let status: RemoteControlStatusChangedNotification =
                serde_json::from_value(notification.params.context("status params")?)?;
            if status.status == RemoteControlConnectionStatus::Disabled {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;
    assert!(matches!(
        timeout(DEFAULT_TIMEOUT, websocket.next()).await?,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));

    let token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .chatgpt_account_id("account_b")
            .chatgpt_user_id("user_b")
            .plan_type("pro"),
    )?;
    let login = app
        .send_chatgpt_auth_tokens_login_request(
            token.clone(),
            "account_b".to_string(),
            Some("pro".to_string()),
        )
        .await?;
    wait_for_response(&mut app, login).await?;
    let enabled = app.send_remote_control_ephemeral_enable_request().await?;
    wait_for_response(&mut app, enabled).await?;
    let (line, reader) = timeout(DEFAULT_TIMEOUT, read_enroll_request(&listener)).await??;
    assert_eq!(
        line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_status(reader.into_inner(), "401 Unauthorized", "expired token").await?;
    let request = timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } = request else {
        anyhow::bail!("expected token refresh after the new owner's 401");
    };
    let refreshed_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .chatgpt_account_id("account_b")
            .chatgpt_user_id("user_b")
            .plan_type("pro")
            .email("refreshed@example.com"),
    )?;
    app.send_response(
        request_id,
        serde_json::to_value(ChatgptAuthTokensRefreshResponse {
            access_token: refreshed_token,
            chatgpt_account_id: "account_b".to_string(),
            chatgpt_plan_type: Some("pro".to_string()),
        })?,
    )
    .await?;
    let (_, reader) = timeout(DEFAULT_TIMEOUT, read_enroll_request(&listener)).await??;
    respond_with_json(
        reader.into_inner(),
        serde_json::json!({
            "server_id": "server-b", "environment_id": "environment-b",
            "remote_control_token": "token-b", "expires_at": "3026-05-22T12:34:56Z",
        }),
    )
    .await?;
    let (stream, _) = timeout(DEFAULT_TIMEOUT, listener.accept()).await??;
    let _websocket = timeout(DEFAULT_TIMEOUT, accept_async(stream)).await??;
    Ok(())
}
