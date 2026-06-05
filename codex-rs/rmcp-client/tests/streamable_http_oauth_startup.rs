mod streamable_http_test_support;

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const VALID_ACCESS_TOKEN: &str = "valid-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const DISCOVERY_UNAVAILABLE_CHILD_SERVER_URL_ENV: &str =
    "MCP_TEST_OAUTH_DISCOVERY_UNAVAILABLE_SERVER_URL";
const DISCOVERY_UNAVAILABLE_ACCESS_TOKEN_ENV: &str =
    "MCP_TEST_OAUTH_DISCOVERY_UNAVAILABLE_ACCESS_TOKEN";
const DISCOVERY_UNAVAILABLE_EXPIRES_IN_MILLIS_ENV: &str =
    "MCP_TEST_OAUTH_DISCOVERY_UNAVAILABLE_EXPIRES_IN_MILLIS";
const DISCOVERY_UNAVAILABLE_EXPECTATION_ENV: &str =
    "MCP_TEST_OAUTH_DISCOVERY_UNAVAILABLE_EXPECTATION";

#[derive(Clone, Copy)]
enum DiscoveryUnavailableExpectation {
    RejectExpired,
    Initialize,
}

impl DiscoveryUnavailableExpectation {
    fn as_str(self) -> &'static str {
        match self {
            Self::RejectExpired => "reject_expired",
            Self::Initialize => "initialize",
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": REFRESHED_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(mcp_response)
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());

    // Credential storage resolves CODEX_HOME from the process environment.
    // Run the client half of the test in an ignored helper test so it can use
    // an isolated home without mutating the parent test runner's environment.
    let status = Command::new(std::env::current_exe()?)
        .args(["oauth_startup_child", "--exact", "--ignored", "--nocapture"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refreshes_expired_persisted_token_before_initialize"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;

    // Save an expired access token with a valid refresh token so startup must
    // refresh before sending the initialize request.
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(SERVER_NAME, &tokens, OAuthCredentialsStoreMode::File)?;

    // This mirrors create_client's transport and initialization setup, except
    // it omits the direct bearer token. Supplying that token would bypass the
    // persisted OAuth credentials and the startup refresh under test.
    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    initialize_client(&client).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn discovery_unavailable_does_not_send_known_expired_access_token() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {EXPIRED_ACCESS_TOKEN}"),
        ))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    run_discovery_unavailable_child(
        &codex_home,
        &server_url,
        EXPIRED_ACCESS_TOKEN,
        /*expires_in_millis*/ 0,
        DiscoveryUnavailableExpectation::RejectExpired,
    )
    .await?;
    server.verify().await;
    assert_discovery_attempted(&server).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn discovery_unavailable_rechecks_expiry_after_discovery() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_delay(Duration::from_millis(250)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {EXPIRED_ACCESS_TOKEN}"),
        ))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    run_discovery_unavailable_child(
        &codex_home,
        &server_url,
        EXPIRED_ACCESS_TOKEN,
        /*expires_in_millis*/ 100,
        DiscoveryUnavailableExpectation::RejectExpired,
    )
    .await?;
    server.verify().await;
    assert_discovery_attempted(&server).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn discovery_unavailable_sends_unexpired_access_token() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {VALID_ACCESS_TOKEN}"),
        ))
        .respond_with(mcp_response)
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    run_discovery_unavailable_child(
        &codex_home,
        &server_url,
        VALID_ACCESS_TOKEN,
        /*expires_in_millis*/ 60_000,
        DiscoveryUnavailableExpectation::Initialize,
    )
    .await?;
    server.verify().await;
    assert_discovery_attempted(&server).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by discovery-unavailable startup tests"]
async fn oauth_discovery_unavailable_child() -> anyhow::Result<()> {
    let server_url = std::env::var(DISCOVERY_UNAVAILABLE_CHILD_SERVER_URL_ENV)?;
    let access_token = std::env::var(DISCOVERY_UNAVAILABLE_ACCESS_TOKEN_ENV)?;
    let expires_in_millis =
        std::env::var(DISCOVERY_UNAVAILABLE_EXPIRES_IN_MILLIS_ENV)?.parse::<u64>()?;
    let expectation = std::env::var(DISCOVERY_UNAVAILABLE_EXPECTATION_ENV)?;

    let response = OAuthTokenResponse::new(
        AccessToken::new(access_token),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(now_millis().saturating_add(expires_in_millis)),
    };
    save_oauth_tokens(SERVER_NAME, &tokens, OAuthCredentialsStoreMode::File)?;

    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await;

    match expectation.as_str() {
        "reject_expired" => {
            let error = match client {
                Ok(_) => panic!("known-expired access token should be rejected"),
                Err(error) => error,
            };
            assert_eq!(
                error.to_string(),
                format!(
                    "stored OAuth access token for MCP server `{SERVER_NAME}` is expired and OAuth metadata discovery is unavailable"
                )
            );
        }
        "initialize" => initialize_client(&client?).await?,
        expectation => anyhow::bail!("unexpected startup expectation: {expectation}"),
    }
    Ok(())
}

async fn run_discovery_unavailable_child(
    codex_home: &TempDir,
    server_url: &str,
    access_token: &str,
    expires_in_millis: u64,
    expectation: DiscoveryUnavailableExpectation,
) -> anyhow::Result<()> {
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_discovery_unavailable_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(DISCOVERY_UNAVAILABLE_CHILD_SERVER_URL_ENV, server_url)
        .env(DISCOVERY_UNAVAILABLE_ACCESS_TOKEN_ENV, access_token)
        .env(
            DISCOVERY_UNAVAILABLE_EXPIRES_IN_MILLIS_ENV,
            expires_in_millis.to_string(),
        )
        .env(DISCOVERY_UNAVAILABLE_EXPECTATION_ENV, expectation.as_str())
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth discovery unavailable child failed: {status}"
    );
    Ok(())
}

async fn assert_discovery_attempted(server: &MockServer) -> anyhow::Result<()> {
    let requests = server
        .received_requests()
        .await
        .ok_or_else(|| anyhow::anyhow!("request recording is unavailable"))?;
    assert!(
        requests
            .iter()
            .any(|request| request.method.as_str() == "GET"),
        "OAuth metadata discovery was not attempted"
    );
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

fn mcp_response(request: &Request) -> ResponseTemplate {
    let body: Value = match request.body_json() {
        Ok(body) => body,
        Err(error) => {
            return ResponseTemplate::new(400)
                .set_body_string(format!("invalid JSON-RPC request: {error}"));
        }
    };
    match body.get("method").and_then(Value::as_str) {
        Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": body.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "protocolVersion": body
                    .pointer("/params/protocolVersion")
                    .cloned()
                    .unwrap_or_else(|| json!("2025-06-18")),
                "capabilities": {},
                "serverInfo": {
                    "name": "oauth-startup-test",
                    "version": "0.0.0-test",
                },
            },
        })),
        Some("notifications/initialized") => ResponseTemplate::new(202),
        method => ResponseTemplate::new(400)
            .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
    }
}
