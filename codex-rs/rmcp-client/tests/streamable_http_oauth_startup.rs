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
use pretty_assertions::assert_eq;
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

use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::initialize_client;
use streamable_http_test_support::initialize_client_with_timeout;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const REVOKED_ACCESS_TOKEN: &str = "revoked-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SCENARIO";
const INVALID_TOKEN_CHALLENGE: &str = r#"Bearer error="invalid_token""#;
const RETRY_REQUIRED_ERROR: &str =
    "MCP OAuth access token was rejected; credentials refreshed, retry the request";

const SCENARIO_INITIALIZE_SUCCEEDS: &str = "initialize_succeeds";
const SCENARIO_INITIALIZE_FAILS: &str = "initialize_fails";
const SCENARIO_INITIALIZE_TIMES_OUT: &str = "initialize_times_out";
const SCENARIO_TOOL_CALL_RECOVERS: &str = "tool_call_recovers";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server).await;
    mount_successful_initialize(&server, REFRESHED_ACCESS_TOKEN).await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = run_child_test(&codex_home, &server_url, "oauth_startup_child").await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialize_invalid_token_refreshes_and_retries_once() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server).await;
    mount_invalid_token_initialize(&server, REVOKED_ACCESS_TOKEN).await;
    mount_successful_initialize(&server, REFRESHED_ACCESS_TOKEN).await;

    run_scenario_child(&server, SCENARIO_INITIALIZE_SUCCEEDS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialize_invalid_token_retry_stops_after_second_401() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server).await;
    mount_invalid_token_initialize(&server, REVOKED_ACCESS_TOKEN).await;
    mount_invalid_token_initialize(&server, REFRESHED_ACCESS_TOKEN).await;

    run_scenario_child(&server, SCENARIO_INITIALIZE_FAILS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialize_invalid_token_retry_uses_original_timeout() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_invalid_token_initialize(&server, REVOKED_ACCESS_TOKEN).await;
    mount_refresh(
        &server,
        successful_refresh_response().set_delay(Duration::from_secs(1)),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    run_scenario_child(&server, SCENARIO_INITIALIZE_TIMES_OUT).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialize_generic_401_does_not_refresh_or_retry() -> anyhow::Result<()> {
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
            ResponseTemplate::new(401)
                .insert_header("www-authenticate", r#"Bearer realm="example""#),
        )
        .expect(1)
        .mount(&server)
        .await;

    run_scenario_child(&server, SCENARIO_INITIALIZE_FAILS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn initialized_notification_invalid_token_does_not_refresh_or_retry() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_unexpected_refresh(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REVOKED_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body = request_body(request);
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(401)
                    .insert_header("www-authenticate", INVALID_TOKEN_CHALLENGE),
                method => unexpected_method(method),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    run_scenario_child(&server, SCENARIO_INITIALIZE_FAILS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tool_call_invalid_token_refreshes_without_replay_and_next_call_succeeds()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REVOKED_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body = request_body(request);
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/call") => ResponseTemplate::new(401)
                    .insert_header("www-authenticate", INVALID_TOKEN_CHALLENGE),
                method => unexpected_method(method),
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body = request_body(request);
            match body.get("method").and_then(Value::as_str) {
                Some("tools/call") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": expected_echo_result("caller retry"),
                })),
                method => unexpected_method(method),
            }
        })
        .expect(1)
        .mount(&server)
        .await;

    run_scenario_child(&server, SCENARIO_TOOL_CALL_RECOVERS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refreshes_expired_persisted_token_before_initialize"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    save_test_oauth_tokens(&server_url, EXPIRED_ACCESS_TOKEN, /*expires_at*/ 0)?;

    let client = new_oauth_client(&server_url).await?;
    initialize_client(&client).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by OAuth invalid_token parent tests"]
async fn oauth_invalid_token_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let scenario = std::env::var(CHILD_SCENARIO_ENV)?;
    save_test_oauth_tokens(&server_url, REVOKED_ACCESS_TOKEN, future_expiry())?;

    let client = new_oauth_client(&server_url).await?;
    match scenario.as_str() {
        SCENARIO_INITIALIZE_SUCCEEDS => initialize_client(&client).await?,
        SCENARIO_INITIALIZE_FAILS => {
            initialize_client(&client).await.unwrap_err();
        }
        SCENARIO_INITIALIZE_TIMES_OUT => {
            let error = initialize_client_with_timeout(&client, Duration::from_millis(500))
                .await
                .unwrap_err();
            assert_eq!(
                error.to_string(),
                "timed out handshaking with MCP server after 500ms"
            );
        }
        SCENARIO_TOOL_CALL_RECOVERS => {
            initialize_client(&client).await?;
            let error = call_echo_tool(&client, "must not be replayed")
                .await
                .unwrap_err();
            assert_eq!(error.to_string(), RETRY_REQUIRED_ERROR);

            let result = call_echo_tool(&client, "caller retry").await?;
            assert_eq!(result, expected_echo_result("caller retry"));
        }
        other => anyhow::bail!("unknown OAuth invalid_token test scenario: {other}"),
    }
    Ok(())
}

async fn run_scenario_child(server: &MockServer, scenario: &str) -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_invalid_token_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .env(CHILD_SCENARIO_ENV, scenario)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth invalid_token child failed: {status}"
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
        .expect(1..=2)
        .mount(server)
        .await;
}

async fn mount_successful_refresh(server: &MockServer) {
    mount_refresh(server, successful_refresh_response()).await;
}

async fn mount_refresh(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN}"
        )))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

fn successful_refresh_response() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": REFRESHED_ACCESS_TOKEN,
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": REFRESH_TOKEN,
    }))
}

async fn mount_unexpected_refresh(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(server)
        .await;
}

async fn mount_invalid_token_initialize(server: &MockServer, access_token: &str) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {access_token}")))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", INVALID_TOKEN_CHALLENGE),
        )
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_successful_initialize(server: &MockServer, access_token: &str) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {access_token}")))
        .respond_with(|request: &Request| {
            let body = request_body(request);
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                method => unexpected_method(method),
            }
        })
        .expect(2)
        .mount(server)
        .await;
}

fn initialize_response(body: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
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
                "name": "oauth-invalid-token-test",
                "version": "0.0.0-test",
            },
        },
    }))
}

fn unexpected_method(method: Option<&str>) -> ResponseTemplate {
    ResponseTemplate::new(400).set_body_string(format!("unexpected JSON-RPC method: {method:?}"))
}

#[expect(clippy::expect_used)]
fn request_body(request: &Request) -> Value {
    request.body_json().expect("valid JSON-RPC request")
}

async fn run_child_test(
    codex_home: &TempDir,
    server_url: &str,
    test_name: &str,
) -> anyhow::Result<std::process::ExitStatus> {
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

async fn call_echo_tool(
    client: &RmcpClient,
    message: &str,
) -> anyhow::Result<rmcp::model::CallToolResult> {
    client
        .call_tool(
            "echo".to_string(),
            Some(json!({ "message": message })),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await
}

fn future_expiry() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.saturating_add(Duration::from_secs(7200)).as_millis() as u64
}
