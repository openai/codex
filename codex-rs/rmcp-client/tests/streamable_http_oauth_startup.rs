mod streamable_http_test_support;

use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::McpLoginRequirement;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::determine_streamable_http_auth_status;
use codex_rmcp_client::is_authentication_required_error;
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
const ROTATED_REFRESH_TOKEN: &str = "rotated-refresh-token";
const FINAL_ACCESS_TOKEN: &str = "final-access-token";
const FINAL_REFRESH_TOKEN: &str = "final-refresh-token";
const REJECTED_RETRY_ACCESS_TOKEN: &str = "rejected-retry-access-token";
const REJECTED_RETRY_REFRESH_TOKEN: &str = "rejected-retry-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const UNREFRESHABLE_SERVER_URL: &str = "https://unrefreshable.example/mcp";
const UNEXPIRED_SERVER_URL: &str = "https://unexpired.example/mcp";
const REFRESHABLE_SERVER_URL: &str = "https://refreshable.example/mcp";

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
async fn recovers_initialization_and_operation_401_once() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_refresh(
        &server,
        REFRESH_TOKEN,
        REFRESHED_ACCESS_TOKEN,
        ROTATED_REFRESH_TOKEN,
    )
    .await;
    mount_refresh(
        &server,
        ROTATED_REFRESH_TOKEN,
        FINAL_ACCESS_TOKEN,
        FINAL_REFRESH_TOKEN,
    )
    .await;
    mount_refresh(
        &server,
        FINAL_REFRESH_TOKEN,
        REJECTED_RETRY_ACCESS_TOKEN,
        REJECTED_RETRY_REFRESH_TOKEN,
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {EXPIRED_ACCESS_TOKEN}"),
        ))
        .respond_with(ResponseTemplate::new(401))
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
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(401),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {FINAL_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body.get("method").and_then(Value::as_str) {
                Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": { "tools": [] },
                })),
                Some("resources/list") => ResponseTemplate::new(401)
                    .insert_header("www-authenticate", "Bearer realm=\"mcp\""),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REJECTED_RETRY_ACCESS_TOKEN}"),
        ))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", "Bearer realm=\"mcp\""),
        )
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_401_recovery_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .status()
        .await?;
    assert!(status.success(), "OAuth recovery child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejected_initialize_retry_requires_reauthentication() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_refresh(
        &server,
        REFRESH_TOKEN,
        REFRESHED_ACCESS_TOKEN,
        ROTATED_REFRESH_TOKEN,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {EXPIRED_ACCESS_TOKEN}"),
        ))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", "Bearer realm=\"mcp\""),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(
            ResponseTemplate::new(401).insert_header("www-authenticate", "Bearer realm=\"mcp\""),
        )
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_rejected_initialize_retry_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth rejected-retry child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn operation_timeout_bounds_unauthorized_refresh_wait() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a"],
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
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(/*millis*/ 500))
                .set_body_json(json!({
                    "access_token": REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": ROTATED_REFRESH_TOKEN,
                    "scope": "scope-a",
                })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {EXPIRED_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => initialize_response(&body),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(401),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        // The later operation can either observe the completed background refresh immediately
        // (three requests with the old token) or race it once before adopting the refreshed
        // credentials (four). The exact provider-refresh and refreshed-token expectations below
        // still require both paths to converge after one rotation.
        .expect(3..=4)
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
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": { "tools": [] },
            }))
        })
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_401_timeout_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .status()
        .await?;
    assert!(status.success(), "OAuth timeout child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reports_auth_status_for_persisted_credentials() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let status = Command::new(std::env::current_exe()?)
        .args([
            "persisted_credentials_auth_status_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .status()
        .await?;

    assert!(
        status.success(),
        "persisted credentials auth status child failed: {status}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn identifies_expired_unrefreshable_token_startup_error() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "expired_unrefreshable_startup_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .status()
        .await?;

    assert!(
        status.success(),
        "expired OAuth startup child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by reports_auth_status_for_persisted_credentials"]
async fn persisted_credentials_auth_status_child() -> anyhow::Result<()> {
    let first_login_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", first_login_server.uri()),
            "token_endpoint": format!("{}/oauth/token", first_login_server.uri()),
        })))
        .expect(1)
        .mount(&first_login_server)
        .await;

    let status = auth_status(&format!("{}/mcp", first_login_server.uri())).await?;
    assert_eq!(status, McpAuthState::LoggedOut(McpLoginRequirement::Login));
    first_login_server.verify().await;

    let response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: UNREFRESHABLE_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(UNREFRESHABLE_SERVER_URL).await?;
    assert_eq!(
        status,
        McpAuthState::LoggedOut(McpLoginRequirement::Reauthentication)
    );

    let response = OAuthTokenResponse::new(
        AccessToken::new("unexpired-access-token".to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: UNEXPIRED_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        // Keep this outside the 60-second proactive refresh guard band. The test is checking a
        // healthy persisted access token, not the boundary where a refresh becomes necessary.
        expires_at: Some(now.saturating_add(/*rhs*/ 120_000)),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(UNEXPIRED_SERVER_URL).await?;
    assert_eq!(status, McpAuthState::OAuth);

    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: REFRESHABLE_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(REFRESHABLE_SERVER_URL).await?;
    assert_eq!(status, McpAuthState::OAuth);
    Ok(())
}

async fn auth_status(server_url: &str) -> anyhow::Result<McpAuthState> {
    determine_streamable_http_auth_status(
        SERVER_NAME,
        server_url,
        /*bearer_token_env_var*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .await
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
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

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
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    initialize_client(&client).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by identifies_expired_unrefreshable_token_startup_error"]
async fn expired_unrefreshable_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let client = RmcpClient::new_streamable_http_client(
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
    .await?;

    let error = initialize_client(&client)
        .await
        .expect_err("expired token without a refresh token should fail startup");
    assert!(is_authentication_required_error(&error));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by recovers_initialization_and_operation_401_once"]
async fn oauth_401_recovery_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let client = refreshable_oauth_client(&server_url).await?;
    initialize_client(&client).await?;
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(/*secs*/ 5)))
        .await?;
    assert!(tools.tools.is_empty());

    let error = client
        .list_resources(/*params*/ None, Some(Duration::from_secs(/*secs*/ 5)))
        .await
        .expect_err("a rejected one-shot OAuth retry should require reauthentication");
    assert!(is_authentication_required_error(&error));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by rejected_initialize_retry_requires_reauthentication"]
async fn oauth_rejected_initialize_retry_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let client = refreshable_oauth_client(&server_url).await?;
    let error = initialize_client(&client)
        .await
        .expect_err("a rejected initialize retry should require reauthentication");
    assert!(is_authentication_required_error(&error));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by operation_timeout_bounds_unauthorized_refresh_wait"]
async fn oauth_401_timeout_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let client = refreshable_oauth_client(&server_url).await?;
    initialize_client(&client).await?;

    let started_at = Instant::now();
    let error = client
        .list_tools(
            /*params*/ None,
            Some(Duration::from_millis(/*millis*/ 50)),
        )
        .await
        .expect_err("operation deadline should expire before the delayed refresh");
    assert!(
        error.to_string().contains("timed out awaiting tools/list"),
        "unexpected operation error: {error:#}"
    );
    assert!(
        started_at.elapsed() < Duration::from_millis(/*millis*/ 400),
        "operation waited for the OAuth provider instead of its own deadline"
    );

    // The caller stopped waiting, but the owned refresh transaction must finish and update the
    // shared manager. A later operation should then use the refreshed token without another
    // provider request.
    tokio::time::sleep(Duration::from_millis(/*millis*/ 600)).await;
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(/*secs*/ 5)))
        .await?;
    assert!(tools.tools.is_empty());
    Ok(())
}

async fn refreshable_oauth_client(server_url: &str) -> anyhow::Result<RmcpClient> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
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
    )?;

    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    Ok(client)
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
            "expires_in": 7200,
            "refresh_token": response_refresh_token,
            "scope": "scope-a",
        })))
        .expect(1)
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
            "capabilities": {},
            "serverInfo": {
                "name": "oauth-401-recovery-test",
                "version": "0.0.0-test",
            },
        },
    }))
}
