mod streamable_http_test_support;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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
const REVOKED_ACCESS_TOKEN: &str = "revoked-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const INVALID_TOKEN_CHALLENGE: &str = r#"Bearer error="invalid_token""#;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
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
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
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
        })
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = run_child_test(&codex_home, &server_url, "oauth_startup_child").await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialize_401_invalid_token_does_not_refresh_or_retry() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_unexpected_refresh(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REVOKED_ACCESS_TOKEN}"),
        ))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", INVALID_TOKEN_CHALLENGE),
        )
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = run_child_test(
        &codex_home,
        &server_url,
        "initialize_401_invalid_token_child",
    )
    .await?;
    assert!(
        status.success(),
        "OAuth initialize 401 child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tool_call_401_invalid_token_does_not_refresh_or_retry() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_unexpected_refresh(&server).await;

    let tool_call_count = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REVOKED_ACCESS_TOKEN}"),
        ))
        .respond_with({
            let tool_call_count = Arc::clone(&tool_call_count);
            move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                match body.get("method").and_then(Value::as_str) {
                    Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id").cloned().unwrap_or(Value::Null),
                        "result": {
                            "protocolVersion": body
                                .pointer("/params/protocolVersion")
                                .cloned()
                                .unwrap_or_else(|| json!("2025-06-18")),
                            "capabilities": {
                                "tools": {},
                            },
                            "serverInfo": {
                                "name": "oauth-reactive-401-test",
                                "version": "0.0.0-test",
                            },
                        },
                    })),
                    Some("notifications/initialized") => ResponseTemplate::new(202),
                    Some("tools/call") => {
                        tool_call_count.fetch_add(1, Ordering::SeqCst);
                        ResponseTemplate::new(401)
                            .insert_header("www-authenticate", INVALID_TOKEN_CHALLENGE)
                    }
                    method => ResponseTemplate::new(400)
                        .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
                }
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = run_child_test(
        &codex_home,
        &server_url,
        "tool_call_401_invalid_token_child",
    )
    .await?;
    assert!(
        status.success(),
        "OAuth tool-call 401 child failed: {status}"
    );
    assert_eq!(tool_call_count.load(Ordering::SeqCst), 1);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refreshes_expired_persisted_token_before_initialize"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;

    // Save an expired access token with a valid refresh token so startup must
    // refresh before sending the initialize request.
    save_test_oauth_tokens(&server_url, EXPIRED_ACCESS_TOKEN, /*expires_at*/ 0)?;

    let client = new_oauth_client(&server_url).await?;
    initialize_client(&client).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by initialize_401_invalid_token_does_not_refresh_or_retry"]
async fn initialize_401_invalid_token_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    save_test_oauth_tokens(&server_url, REVOKED_ACCESS_TOKEN, future_expiry())?;

    let client = new_oauth_client(&server_url).await?;
    let error = initialize_client(&client).await.unwrap_err();
    let error_message = error.to_string();
    assert!(
        error_message.contains("handshaking with MCP server failed: Send message error")
            && error_message.contains("Auth required")
            && error_message.contains("when send initialize request"),
        "unexpected initialize error: {error:#}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by tool_call_401_invalid_token_does_not_refresh_or_retry"]
async fn tool_call_401_invalid_token_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    save_test_oauth_tokens(&server_url, REVOKED_ACCESS_TOKEN, future_expiry())?;

    let client = new_oauth_client(&server_url).await?;
    initialize_client(&client).await?;
    let error = client
        .call_tool(
            "echo".to_string(),
            Some(json!({ "message": "should not be replayed" })),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("Transport send error")
            && error.to_string().contains("Auth required"),
        "unexpected tool-call error: {error:#}"
    );
    Ok(())
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_unexpected_refresh(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(server)
        .await;
}

async fn run_child_test(
    codex_home: &TempDir,
    server_url: &str,
    test_name: &str,
) -> anyhow::Result<std::process::ExitStatus> {
    // Credential storage resolves CODEX_HOME from the process environment.
    // Run the client half of the test in an ignored helper test so it can use
    // an isolated home without mutating the parent test runner's environment.
    Ok(Command::new(std::env::current_exe()?)
        .args([test_name, "--exact", "--ignored", "--nocapture"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?)
}

fn save_test_oauth_tokens(
    server_url: &str,
    access_token: &str,
    expires_at: u64,
) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(access_token.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(expires_at),
    };
    save_oauth_tokens(SERVER_NAME, &tokens, OAuthCredentialsStoreMode::File)?;
    Ok(())
}

async fn new_oauth_client(server_url: &str) -> anyhow::Result<RmcpClient> {
    // This mirrors create_client's transport and initialization setup, except
    // it omits the direct bearer token. Supplying that token would bypass the
    // persisted OAuth credentials and the startup refresh under test.
    RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await
}

fn future_expiry() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.saturating_add(Duration::from_secs(7200)).as_millis() as u64
}
