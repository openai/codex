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

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_EXTERNAL_UPDATE_SERVER_URL";
const OLD_ACCESS_TOKEN: &str = "old-access-token";
const OLD_REFRESH_TOKEN: &str = "old-refresh-token";
const EXTERNAL_ACCESS_TOKEN: &str = "external-access-token";
const EXTERNAL_REFRESH_TOKEN: &str = "external-refresh-token";
const STALE_REFRESHED_ACCESS_TOKEN: &str = "stale-refreshed-access-token";
const STALE_ROTATED_REFRESH_TOKEN: &str = "stale-rotated-refresh-token";
const REFRESH_SKEW: Duration = Duration::from_secs(30);

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
async fn running_client_keeps_and_can_persist_over_external_oauth_update() -> anyhow::Result<()> {
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
            "refresh_token={OLD_REFRESH_TOKEN}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": STALE_REFRESHED_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": STALE_ROTATED_REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let tool_call_count = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with({
            let tool_call_count = Arc::clone(&tool_call_count);
            move |request: &Request| {
                let body: Value = request.body_json().expect("valid JSON-RPC request");
                let method = body.get("method").and_then(Value::as_str);
                let authorization = request
                    .headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok());
                let expected_access_token = match method {
                    Some("tools/call") if tool_call_count.fetch_add(1, Ordering::SeqCst) == 1 => {
                        STALE_REFRESHED_ACCESS_TOKEN
                    }
                    _ => OLD_ACCESS_TOKEN,
                };
                if authorization != Some(format!("Bearer {expected_access_token}").as_str()) {
                    return ResponseTemplate::new(401);
                }

                match method {
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
                                "name": "oauth-external-update-test",
                                "version": "0.0.0-test",
                            },
                        },
                    })),
                    Some("notifications/initialized") => ResponseTemplate::new(202),
                    Some("tools/call") => ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": body.get("id").cloned().unwrap_or(Value::Null),
                        "result": {
                            "content": [],
                            "isError": false,
                        },
                    })),
                    method => ResponseTemplate::new(400)
                        .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
                }
            }
        })
        .expect(4)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_external_update_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth external update child failed: {status}"
    );
    assert_eq!(tool_call_count.load(Ordering::SeqCst), 2);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by running_client_keeps_and_can_persist_over_external_oauth_update"]
async fn oauth_external_update_child() -> anyhow::Result<()> {
    let server_url = std::env::var(EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV)?;
    let old_expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .checked_add(REFRESH_SKEW + Duration::from_secs(5))
        .expect("test expiry should fit")
        .as_millis() as u64;
    save_test_tokens(
        &server_url,
        OLD_ACCESS_TOKEN,
        OLD_REFRESH_TOKEN,
        old_expires_at,
    )?;

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

    let external_expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .checked_add(Duration::from_secs(7200))
        .expect("test expiry should fit")
        .as_millis() as u64;
    save_test_tokens(
        &server_url,
        EXTERNAL_ACCESS_TOKEN,
        EXTERNAL_REFRESH_TOKEN,
        external_expires_at,
    )?;

    client
        .call_tool(
            "first-call-after-external-update".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    let refresh_at = UNIX_EPOCH
        + Duration::from_millis(old_expires_at)
            .saturating_sub(REFRESH_SKEW)
            .saturating_add(Duration::from_millis(250));
    if let Ok(wait) = refresh_at.duration_since(SystemTime::now()) {
        tokio::time::sleep(wait).await;
    }

    client
        .call_tool(
            "second-call-after-stale-refresh".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    let credentials = std::fs::read_to_string(
        std::path::Path::new(&std::env::var("CODEX_HOME")?).join(".credentials.json"),
    )?;
    assert!(credentials.contains(STALE_REFRESHED_ACCESS_TOKEN));
    assert!(credentials.contains(STALE_ROTATED_REFRESH_TOKEN));
    assert!(!credentials.contains(EXTERNAL_ACCESS_TOKEN));
    assert!(!credentials.contains(EXTERNAL_REFRESH_TOKEN));
    Ok(())
}

fn save_test_tokens(
    server_url: &str,
    access_token: &str,
    refresh_token: &str,
    expires_at: u64,
) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(access_token.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(refresh_token.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    save_oauth_tokens(
        SERVER_NAME,
        &StoredOAuthTokens {
            server_name: SERVER_NAME.to_string(),
            url: server_url.to_string(),
            client_id: "test-client-id".to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at: Some(expires_at),
        },
        OAuthCredentialsStoreMode::File,
    )
}
