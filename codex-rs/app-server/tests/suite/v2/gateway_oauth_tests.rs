//! Gateway OAuth RPCs expose login progress, cancellation, and initialization policy.

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthChangedNotification;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn gateway_config(oauth: &MockServer) -> String {
    format!(
        r#"
model_provider = "gateway"
[features]
secret_auth_storage = true
[model_providers.gateway]
name = "Test gateway"
base_url = "{url}/v1"
wire_api = "responses"
[model_providers.gateway.gateway_oauth]
authorization_url = "{url}/authorize"
token_url = "{url}/token"
client_id = "app-server-explicit-test"
delivery = {{ kind = "header", name = "X-Gateway-Authorization" }}
"#,
        url = oauth.uri()
    )
}

async fn gateway_server(home: &std::path::Path) -> Result<TestAppServer> {
    let mut server = TestAppServer::builder()
        .with_codex_home(home)
        .build()
        .await?;
    server
        .initialize_with_capabilities(
            ClientInfo {
                name: "gateway-test".to_string(),
                title: None,
                version: "1".to_string(),
            },
            Some(InitializeCapabilities {
                explicit_gateway_oauth: true,
                experimental_api: true,
                ..Default::default()
            }),
        )
        .await?;
    Ok(server)
}

async fn read_gateway(server: &mut TestAppServer) -> Result<GatewayOAuthReadResponse> {
    server
        .request(|id| ClientRequest::GatewayOAuthRead {
            request_id: id,
            params: None,
        })
        .await
}

async fn cancel_gateway(server: &mut TestAppServer) -> Result<GatewayOAuthCancelResponse> {
    server
        .request(|id| ClientRequest::GatewayOAuthCancel {
            request_id: id,
            params: None,
        })
        .await
}

async fn authorization_url(server: &mut TestAppServer) -> Result<url::Url> {
    timeout(TIMEOUT, async {
        loop {
            let changed: GatewayOAuthChangedNotification = server
                .read_notification("account/gatewayOAuth/changed")
                .await?;
            if let Some(url) = changed.auth_url {
                return Ok(url::Url::parse(&url)?);
            }
        }
    })
    .await?
}

const TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);

#[tokio::test]
async fn model_list_requests_restart_after_gateway_provider_changes() -> Result<()> {
    let oauth = MockServer::start().await;
    let original = gateway_config(&oauth);
    for replacement in [
        String::new(), // Switch to the default provider without gateway OAuth.
        original.replace("/authorize", "/replacement-authorize"),
    ] {
        let home = TempDir::new()?;
        let config_path = home.path().join("config.toml");
        std::fs::write(&config_path, &original)?;
        let mut server = gateway_server(home.path()).await?;
        std::fs::write(&config_path, replacement)?;
        let models = server
            .send_raw_request("model/list", Some(serde_json::json!({})))
            .await?;
        let error = timeout(
            TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(models)),
        )
        .await??;
        assert_eq!(error.error, JSONRPCErrorError {
            code: -32600,
            message: "Model provider settings changed. Restart Codex to apply them, then retry fetching the model list".to_string(),
            data: None,
        });
    }
    Ok(())
}

#[tokio::test]
async fn token_exchange_failure_completes_login_and_updates_readiness() -> Result<()> {
    let home = TempDir::new()?;
    let oauth = MockServer::start().await;
    let secret = "issuer-query-credential";
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=accepted"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 400)
                .insert_header("x-request-id", secret)
                .set_body_json(serde_json::json!({
                    "error": "invalid_grant", "error_description": secret,
                })),
        )
        .expect(/*r*/ 1)
        .mount(&oauth)
        .await;
    std::fs::write(
        home.path().join("config.toml"),
        gateway_config(&oauth).replace(
            "/authorize\"",
            &format!("/authorize?issuer_secret={secret}\""),
        ),
    )?;
    let mut server = gateway_server(home.path()).await?;
    let login = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    let authorization_url = authorization_url(&mut server).await?;
    let query = authorization_url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let mut callback = url::Url::parse(query.get("redirect_uri").context("callback URI")?)?;
    callback
        .query_pairs_mut()
        .append_pair("code", "accepted")
        .append_pair("state", query.get("state").context("OAuth state")?);
    codex_login::default_client::create_client_without_request_logging()
        .get(callback.as_str())
        .send()
        .await?
        .error_for_status()?;
    let error = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(login)),
    )
    .await??;
    let message = "Gateway sign-in failed; check the gateway configuration and credential store.";
    assert_eq!(error.error.message, message);
    let changed = timeout(
        TIMEOUT,
        server.read_stream_until_matching_notification("gateway login failure", |notification| {
            notification.method == "account/gatewayOAuth/changed"
                && notification
                    .params
                    .as_ref()
                    .is_some_and(|params| params["status"] == "failed")
        }),
    )
    .await??;
    assert_eq!(
        serde_json::from_value::<GatewayOAuthChangedNotification>(
            changed.params.context("gateway status")?
        )?,
        GatewayOAuthChangedNotification {
            provider_id: "gateway".into(),
            status: GatewayOAuthStatus::Failed,
            auth_url: None,
            error: Some(message.into()),
        }
    );
    let state: GatewayOAuthReadResponse = read_gateway(&mut server).await?;
    assert_eq!(
        state,
        GatewayOAuthReadResponse {
            provider_id: "gateway".to_string(),
            provider_name: "Test gateway".to_string(),
            required: true,
            status: Some(GatewayOAuthStatus::Failed),
            error: Some(error.error.message),
        }
    );
    Ok(())
}

#[tokio::test]
async fn read_and_cancel_without_gateway_oauth() -> Result<()> {
    let home = TempDir::new()?;
    let mut server = gateway_server(home.path()).await?;
    let state: GatewayOAuthReadResponse = read_gateway(&mut server).await?;
    assert_eq!(
        (state.required, state.status, state.error),
        (false, None, None)
    );
    let response: GatewayOAuthCancelResponse = cancel_gateway(&mut server).await?;
    assert_eq!(response, GatewayOAuthCancelResponse {});
    let login = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    let error = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(login)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "The current provider does not use gateway OAuth"
    );
    Ok(())
}

#[tokio::test]
async fn primary_account_is_required_before_gateway_login() -> Result<()> {
    let home = TempDir::new()?;
    let oauth = MockServer::start().await;
    let config = gateway_config(&oauth).replace(
        "wire_api = \"responses\"",
        "wire_api = \"responses\"\nrequires_openai_auth = true",
    );
    std::fs::write(home.path().join("config.toml"), config)?;
    let mut server = gateway_server(home.path()).await?;
    let login = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    let error = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(login)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "Sign in to your primary account before signing in to the gateway"
    );
    assert!(oauth.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn read_is_passive_and_canceled_login_can_be_retried_immediately() -> Result<()> {
    let home = TempDir::new()?;
    let oauth = MockServer::start().await;
    let config = gateway_config(&oauth);
    std::fs::write(home.path().join("config.toml"), &config)?;
    let mut server = gateway_server(home.path()).await?;
    let state: GatewayOAuthReadResponse = read_gateway(&mut server).await?;
    assert_eq!(
        state,
        GatewayOAuthReadResponse {
            provider_id: "gateway".to_string(),
            provider_name: "Test gateway".to_string(),
            required: true,
            status: Some(GatewayOAuthStatus::NotReady),
            error: None
        }
    );
    assert!(oauth.received_requests().await.unwrap().is_empty());
    let expected = GatewayOAuthChangedNotification {
        provider_id: "gateway".to_string(),
        status: GatewayOAuthStatus::NotReady,
        auth_url: None,
        error: None,
    };
    // Consume the initial readiness change before checking repeated request failures.
    let changed: GatewayOAuthChangedNotification = timeout(
        TIMEOUT,
        server.read_notification("account/gatewayOAuth/changed"),
    )
    .await??;
    assert_eq!(changed, expected);
    for _ in 0..2 {
        let models = server
            .send_raw_request("model/list", Some(serde_json::json!({})))
            .await?;
        let error = timeout(
            TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(models)),
        )
        .await??;
        assert!(error.error.message.contains("Gateway sign-in required"));
        let changed: GatewayOAuthChangedNotification = timeout(
            TIMEOUT,
            server.read_notification("account/gatewayOAuth/changed"),
        )
        .await??;
        assert_eq!(changed, expected);
    }
    let login = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    let url = authorization_url(&mut server).await?;
    assert!(
        url.as_str()
            .starts_with(&format!("{}/authorize?", oauth.uri()))
    );
    let state: GatewayOAuthReadResponse = read_gateway(&mut server).await?;
    assert_eq!(state.status, Some(GatewayOAuthStatus::Started));
    let duplicate = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    let error = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(duplicate)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "Gateway sign-in is already in progress"
    );
    let _: GatewayOAuthCancelResponse = cancel_gateway(&mut server).await?;
    // Retry on the cancel acknowledgment, without waiting for the old login response.
    let retry = server
        .send_raw_request("account/gatewayOAuth/login", /*params*/ None)
        .await?;
    authorization_url(&mut server).await?;
    let _: GatewayOAuthCancelResponse = cancel_gateway(&mut server).await?;
    for request_id in [login, retry] {
        let error = timeout(
            TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(error.error.message, "Gateway sign-in was canceled");
    }
    assert!(oauth.received_requests().await.unwrap().is_empty());

    let state: GatewayOAuthReadResponse = read_gateway(&mut server).await?;
    assert_eq!(
        (state.status, state.error),
        (
            Some(GatewayOAuthStatus::Failed),
            Some("Gateway sign-in was canceled".to_string())
        )
    );

    Ok(())
}

#[tokio::test]
async fn disconnect_cancels_only_the_owning_connections_login() -> Result<()> {
    use super::connection_handling_websocket::connect_websocket;
    use super::connection_handling_websocket::read_error_for_id;
    use super::connection_handling_websocket::read_notification_for_method;
    use super::connection_handling_websocket::read_response_for_id;
    use super::connection_handling_websocket::send_request;
    use super::connection_handling_websocket::spawn_websocket_server;
    let home = TempDir::new()?;
    let oauth = MockServer::start().await;
    std::fs::write(home.path().join("config.toml"), gateway_config(&oauth))?;
    let (mut process, address) = spawn_websocket_server(home.path()).await?;
    let mut owner = connect_websocket(address).await?;
    let mut observer = connect_websocket(address).await?;
    send_request(&mut owner, "initialize", /*id*/ 1, Some(serde_json::json!({"clientInfo": {"name": "gateway-owner", "version": "1"}, "capabilities": {"experimentalApi": true, "explicitGatewayOauth": true}}))).await?;
    read_response_for_id(&mut owner, /*id*/ 1).await?;
    send_request(&mut observer, "initialize", /*id*/ 1, Some(serde_json::json!({"clientInfo": {"name": "gateway-observer", "version": "1"}, "capabilities": {"experimentalApi": true}}))).await?;
    read_response_for_id(&mut observer, /*id*/ 1).await?;
    // A legacy connection cannot undo another connection's explicit opt-in.
    send_request(
        &mut observer,
        "model/list",
        /*id*/ 0,
        Some(serde_json::json!({})),
    )
    .await?;
    let error = read_error_for_id(&mut observer, /*id*/ 0).await?;
    assert!(error.error.message.contains("Gateway sign-in required"));
    send_request(
        &mut owner,
        "account/gatewayOAuth/login",
        /*id*/ 2,
        /*params*/ None,
    )
    .await?;
    // Startup can publish readiness before the login request starts authorization.
    timeout(TIMEOUT, async {
        loop {
            let changed =
                read_notification_for_method(&mut observer, "account/gatewayOAuth/changed").await?;
            if changed
                .params
                .as_ref()
                .and_then(|params| params["status"].as_str())
                == Some("started")
            {
                return anyhow::Ok(());
            }
        }
    })
    .await??;
    send_request(
        &mut observer,
        "account/gatewayOAuth/cancel",
        /*id*/ 2,
        /*params*/ None,
    )
    .await?;
    let error = read_error_for_id(&mut observer, /*id*/ 2).await?;
    assert_eq!(
        error.error.message,
        "Gateway sign-in belongs to another connection"
    );
    owner.close(/*msg*/ None).await?;
    let canceled =
        read_notification_for_method(&mut observer, "account/gatewayOAuth/changed").await?;
    assert_eq!(canceled.params.unwrap()["status"], "failed");
    send_request(
        &mut observer,
        "account/gatewayOAuth/login",
        /*id*/ 3,
        /*params*/ None,
    )
    .await?;
    let started =
        read_notification_for_method(&mut observer, "account/gatewayOAuth/changed").await?;
    assert_eq!(started.params.unwrap()["status"], "started");
    send_request(
        &mut observer,
        "account/gatewayOAuth/cancel",
        /*id*/ 4,
        /*params*/ None,
    )
    .await?;
    // Stop the test process after observing that a new connection can own a new login.
    process.kill().await?;
    Ok(())
}

#[tokio::test]
async fn initialization_selects_legacy_or_explicit_gateway_login() -> Result<()> {
    for explicit in [false, true] {
        let home = TempDir::new()?;
        let oauth = MockServer::start().await;
        // Force automatic authorization to fail before launching a real browser.
        let callback = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = callback.local_addr()?.port();
        std::fs::write(
            home.path().join("config.toml"),
            format!("{}\nredirect_port = {port}\n", gateway_config(&oauth)),
        )?;
        let mut server = TestAppServer::builder()
            .with_codex_home(home.path())
            .build()
            .await?;
        server
            .initialize_with_capabilities(
                ClientInfo {
                    name: "gateway-compat-test".into(),
                    title: None,
                    version: "1".into(),
                },
                Some(InitializeCapabilities {
                    explicit_gateway_oauth: explicit,
                    experimental_api: true,
                    ..Default::default()
                }),
            )
            .await?;
        std::fs::write(
            home.path().join("config.toml"),
            format!("{}\nredirect_port = {port}\n", gateway_config(&oauth))
                .replace("app-server-explicit-test", "gateway-after-initialize"),
        )?;
        let thread = server
            .send_thread_start_request_with_auto_env(ThreadStartParams::default())
            .await?;
        let thread: ThreadStartResponse = server.read_response(thread).await?;
        let turn = server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.thread.id,
                input: vec![UserInput::Text {
                    text: "hello".into(),
                    text_elements: vec![],
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse = server.read_response(turn).await?;
        let completed: TurnCompletedNotification =
            timeout(TIMEOUT, server.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Failed);
        assert_eq!(
            completed.turn.error.context("gateway failure")?.message,
            if explicit {
                "Gateway sign-in required. Choose Sign in or Reconnect, then retry your request"
            } else {
                "Gateway OAuth authentication failed; check the gateway configuration and credential store."
            }
        );
        assert!(oauth.received_requests().await.unwrap().is_empty());
    }
    Ok(())
}
