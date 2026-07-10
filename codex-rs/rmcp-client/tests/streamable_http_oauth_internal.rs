mod streamable_http_test_support;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use codex_config::types::AuthKeyringBackendKind;
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

const SERVER_NAME: &str = "test-streamable-http-oauth-internal";
const SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_INTERNAL_SERVER_URL";
const GET_FAILURE_MARKER_ENV: &str = "MCP_TEST_OAUTH_INTERNAL_GET_FAILURE_MARKER";
const ACCESS_TOKEN_A: &str = "internal-access-a";
const REFRESH_TOKEN_A: &str = "internal-refresh-a";
const ACCESS_TOKEN_B: &str = "internal-access-b";
const REFRESH_TOKEN_B: &str = "internal-refresh-b";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rmcp_owned_get_reports_auth_failure_for_parent_recovery() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let get_failure_marker = codex_home.path().join("get-failure-observed");
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_refresh(&server, REFRESH_TOKEN_A, ACCESS_TOKEN_B, REFRESH_TOKEN_B).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_A}")))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_A}")))
        .respond_with({
            let get_failure_marker = get_failure_marker.clone();
            move |_request: &Request| {
                std::fs::write(&get_failure_marker, b"observed")
                    .expect("record RMCP-owned GET auth failure");
                ResponseTemplate::new(401)
            }
        })
        // RMCP's default SSE reconnect policy is unbounded. The Codex failure latch must make
        // this logical reconnect terminal instead of repeatedly re-entering get_stream with A.
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_B}")))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": { "tools": [] },
                })),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_B}")))
        .respond_with(ResponseTemplate::new(405))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .and(header("authorization", format!("Bearer {ACCESS_TOKEN_B}")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_internal_get_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .env(GET_FAILURE_MARKER_ENV, &get_failure_marker)
        .status()
        .await?;
    anyhow::ensure!(status.success(), "OAuth internal child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by rmcp_owned_get_reports_auth_failure_for_parent_recovery"]
async fn oauth_internal_get_child() -> anyhow::Result<()> {
    let client = create_oauth_client().await?;
    initialize_client(&client).await?;
    wait_for_marker(GET_FAILURE_MARKER_ENV).await?;

    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(/*secs*/ 5)))
        .await?;
    assert!(tools.tools.is_empty());
    client.shutdown().await;
    Ok(())
}

async fn wait_for_marker(env_name: &str) -> anyhow::Result<()> {
    let path = PathBuf::from(
        std::env::var_os(env_name).with_context(|| format!("missing {env_name} environment"))?,
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .with_context(|| format!("timed out waiting for marker {}", path.display()))
}

async fn create_oauth_client() -> anyhow::Result<RmcpClient> {
    let server_url = std::env::var(SERVER_URL_ENV)?;
    save_initial_tokens(&server_url)?;
    RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await
}

fn save_initial_tokens(server_url: &str) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(ACCESS_TOKEN_A.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN_A.to_string())));
    response.set_expires_in(None);
    save_oauth_tokens(
        SERVER_NAME,
        &StoredOAuthTokens {
            server_name: SERVER_NAME.to_string(),
            url: server_url.to_string(),
            client_id: "test-client-id".to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at: None,
        },
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
        })))
        .mount(server)
        .await;
}

async fn mount_refresh(
    server: &MockServer,
    request_refresh_token: &str,
    response_access_token: &str,
    response_refresh_token: &str,
) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={request_refresh_token}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": response_access_token,
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": response_refresh_token,
            "scope": "scope-a",
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn initialize_response(body: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("mcp-session-id", "oauth-internal-session")
        .set_body_json(json!({
            "jsonrpc": "2.0",
            "id": body.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "protocolVersion": body
                    .pointer("/params/protocolVersion")
                    .cloned()
                    .unwrap_or_else(|| json!("2025-06-18")),
                "capabilities": {},
                "serverInfo": {
                    "name": "oauth-internal-test",
                    "version": "0.0.0-test"
                }
            }
        }))
}
