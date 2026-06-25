use std::collections::HashMap;
use std::sync::Arc;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use reqwest::header::HeaderMap;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::OAuthTransportClient;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::oauth::OAuthPersistor;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::request_oauth_token_response;
use crate::oauth::save_oauth_tokens;
use crate::oauth_http_client::OAuthHttpClientAdapter;

const SERVER_NAME: &str = "oauth-transport-response-test";
const SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_RESPONSE_SERVER_URL";
const ACCESS_TOKEN_A: &str = "response-access-a";
const REFRESH_TOKEN_A: &str = "response-refresh-a";
const ACCESS_TOKEN_B: &str = "response-access-b";
const REFRESH_TOKEN_B: &str = "response-refresh-b";

#[tokio::test]
async fn server_response_post_receives_one_shot_oauth_recovery() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN_A}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": ACCESS_TOKEN_B,
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": REFRESH_TOKEN_B,
            "scope": "scope-a",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_A}")))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_B}")))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_transport::tests::server_response_post_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(SERVER_URL_ENV, server_url)
        .status()
        .await?;
    anyhow::ensure!(status.success(), "OAuth response child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test]
#[ignore = "spawned by server_response_post_receives_one_shot_oauth_recovery"]
async fn server_response_post_child() -> anyhow::Result<()> {
    let server_url = std::env::var(SERVER_URL_ENV)?;
    let initial_tokens = initial_tokens(&server_url);
    save_oauth_tokens(
        SERVER_NAME,
        &initial_tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let http_client = Environment::default_for_tests().get_http_client();
    let oauth_http_client = Arc::new(OAuthHttpClientAdapter::new(
        Arc::clone(&http_client),
        HeaderMap::new(),
    ));
    let mut oauth_state =
        OAuthState::new_with_oauth_http_client(server_url.clone(), oauth_http_client).await?;
    oauth_state
        .set_credentials(
            &initial_tokens.client_id,
            request_oauth_token_response(&initial_tokens),
        )
        .await?;
    let manager = match oauth_state {
        OAuthState::Authorized(manager) | OAuthState::Unauthorized(manager) => manager,
        _ => anyhow::bail!("unexpected OAuth state during response test setup"),
    };
    let auth_client = AuthClient::new(
        StreamableHttpClientAdapter::new(
            Arc::clone(&http_client),
            HeaderMap::new(),
            /*auth_provider*/ None,
        ),
        manager,
    );
    let persistor = OAuthPersistor::new(
        SERVER_NAME.to_string(),
        server_url.clone(),
        Arc::clone(&auth_client.auth_manager),
        ResolvedOAuthCredentialStore::File,
        Some(initial_tokens),
    );
    let client = OAuthTransportClient::new(auth_client, persistor);
    let response_message: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": "server-request-1",
        "result": {
            "action": "accept",
            "content": { "confirmed": true }
        }
    }))?;

    let response = client
        .post_message(
            Arc::from(server_url),
            response_message,
            Some(Arc::from("response-session")),
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await?;

    assert!(matches!(response, StreamableHttpPostResponse::Accepted));
    Ok(())
}

fn initial_tokens(server_url: &str) -> StoredOAuthTokens {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(ACCESS_TOKEN_A.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN_A.to_string())));
    response.set_expires_in(None);
    StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: None,
    }
}
