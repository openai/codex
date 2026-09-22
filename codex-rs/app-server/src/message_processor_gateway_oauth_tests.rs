//! Exercise gateway sign-in and managed-provider checks through JSON-RPC with a mock keyring.

use super::ConnectionSessionState;
use super::MessageProcessor;
use super::message_processor_tracing_tests::TEST_CONNECTION_ID;
use super::message_processor_tracing_tests::build_test_processor;
use super::message_processor_tracing_tests::read_response;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::transport::AppServerTransport;
use anyhow::Result;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_core::config::ConfigBuilder;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::test_support::seed_gateway_auth;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn send_request(
    processor: &Arc<MessageProcessor>,
    session: &Arc<ConnectionSessionState>,
    request: serde_json::Value,
) {
    processor
        .process_request(
            TEST_CONNECTION_ID,
            serde_json::from_value(request).expect("JSON-RPC request"),
            &AppServerTransport::Stdio,
            Arc::clone(session),
        )
        .await;
}

#[tokio::test]
async fn gateway_oauth_login_updates_existing_requests() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=accepted"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "new-access", "refresh_token": "new-refresh", "expires_in": 3600
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"
model_provider = "gateway"
[model_providers.gateway]
name = "OpenAI"
requires_openai_auth = true
base_url = "{url}/v1"
wire_api = "responses"
[model_providers.gateway.gateway_oauth]
authorization_url = "{url}/authorize"
token_url = "{url}/token"
client_id = "app-server-login-success"
delivery = {{ kind = "header", name = "X-Gateway-Authorization" }}
"#,
            url = server.uri()
        ),
    )?;
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?,
    );
    let auth_manager = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("primary"),
        home.path().to_path_buf(),
    );
    let observer = seed_gateway_auth(
        &config.model_provider,
        &auth_manager,
        json!({
            "access_token": "old-access", "refresh_token": "old-refresh", "expires_at": null,
        }),
    );
    assert_eq!(observer.resolve_access_token().await?, "old-access");
    let provider = codex_model_provider::create_model_provider(
        config.model_provider.clone(),
        Some(Arc::clone(&auth_manager)),
    );
    assert_eq!(
        provider.api_auth().await?.to_auth_headers()["x-gateway-authorization"],
        "Bearer old-access"
    );
    let (processor, mut outgoing) = build_test_processor(config, auth_manager).await;
    let session = Arc::new(ConnectionSessionState::new(
        crate::transport::ConnectionOrigin::Stdio,
    ));
    send_request(
        &processor,
        &session,
        json!({
            "id": 1, "method": "initialize", "params": {
                "clientInfo": {"name": "gateway-test", "version": "1"}
            }
        }),
    )
    .await;
    let _: InitializeResponse = read_response(&mut outgoing, /*request_id*/ 1).await;
    send_request(
        &processor,
        &session,
        json!({"id": 2, "method": "account/gatewayOAuth/login"}),
    )
    .await;
    let authorization_url = timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            let envelope = outgoing
                .recv()
                .await
                .expect("outgoing channel open during login");
            if let OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(notification),
                ..
            } = envelope
                && let ServerNotification::GatewayOAuthChanged(changed) = notification.notification
                && let Some(url) = changed.auth_url
            {
                assert_eq!(connection_id, TEST_CONNECTION_ID);
                return url;
            }
        }
    })
    .await?;
    let query = url::Url::parse(&authorization_url)?
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let mut callback = url::Url::parse(&query["redirect_uri"])?;
    callback
        .query_pairs_mut()
        .append_pair("code", "accepted")
        .append_pair("state", &query["state"]);
    codex_login::default_client::create_client_without_request_logging()
        .get(callback.as_str())
        .send()
        .await?
        .error_for_status()?;
    let response: GatewayOAuthLoginResponse = read_response(&mut outgoing, /*request_id*/ 2).await;
    assert_eq!(response, GatewayOAuthLoginResponse {});
    send_request(
        &processor,
        &session,
        json!({"id": 3, "method": "account/gatewayOAuth/read"}),
    )
    .await;
    let response: GatewayOAuthReadResponse = read_response(&mut outgoing, /*request_id*/ 3).await;
    assert_eq!(
        response,
        GatewayOAuthReadResponse {
            provider_id: "gateway".to_string(),
            provider_name: "OpenAI".to_string(),
            required: true,
            status: Some(GatewayOAuthStatus::Succeeded),
            error: None,
        }
    );
    assert_eq!(observer.resolve_access_token().await?, "new-access");
    assert_eq!(
        provider.api_auth().await?.to_auth_headers()["x-gateway-authorization"],
        "Bearer new-access"
    );
    processor.shutdown_threads().await;
    processor.drain_background_tasks().await;
    Ok(())
}

#[tokio::test]
async fn gateway_oauth_model_list_checks_requirements_before_refreshing_token() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "refreshed", "expires_in": 3600,
        })))
        .mount(&server)
        .await;
    let home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    config.model_provider_id = "gateway".into();
    config.model_provider = toml::from_str(&format!(
        r#"
name = "Gateway"
base_url = "{url}/v1"
[gateway_oauth]
authorization_url = "{url}/authorize"
token_url = "{url}/token"
client_id = "app-server-requirements-test"
delivery = {{ kind = "header", name = "X-Gateway-Authorization" }}
"#,
        url = server.uri(),
    ))?;
    let auth_manager = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("primary"),
        home.path().to_path_buf(),
    );
    let _gateway = seed_gateway_auth(
        &config.model_provider,
        &auth_manager,
        json!({"access_token": "expired", "refresh_token": "old-refresh", "expires_at": 0}),
    );
    let (processor, mut outgoing) = build_test_processor(Arc::new(config), auth_manager).await;
    let session = Arc::new(ConnectionSessionState::new(
        crate::transport::ConnectionOrigin::Stdio,
    ));
    send_request(
        &processor,
        &session,
        json!({"id": 1, "method": "initialize", "params": {
            "clientInfo": {"name": "gateway-test", "version": "1"},
            "capabilities": {"explicitGatewayOauth": true},
        }}),
    )
    .await;
    let _: InitializeResponse = read_response(&mut outgoing, /*request_id*/ 1).await;
    std::fs::write(
        home.path().join("requirements.toml"),
        "model_provider = 'openai'",
    )?;
    send_request(
        &processor,
        &session,
        json!({"id": 2, "method": "model/list", "params": {}}),
    )
    .await;
    let error = timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if let OutgoingEnvelope::ToConnection {
                message: OutgoingMessage::Error(error),
                ..
            } = outgoing
                .recv()
                .await
                .expect("outgoing channel open during request")
            {
                break error.error;
            }
        }
    })
    .await?;
    assert_eq!(error, JSONRPCErrorError {
        code: -32600,
        message: "failed to load configuration: Your organization's required model provider settings changed. Restart Codex to apply them; this request was not sent".to_string(),
        data: None,
    });
    assert!(server.received_requests().await.unwrap().is_empty());
    processor.shutdown_threads().await;
    processor.drain_background_tasks().await;
    Ok(())
}
