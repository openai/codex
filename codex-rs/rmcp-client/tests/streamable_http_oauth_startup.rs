mod streamable_http_test_support;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

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
const SECOND_REFRESHED_ACCESS_TOKEN: &str = "second-refreshed-access-token";
const THIRD_REFRESHED_ACCESS_TOKEN: &str = "third-refreshed-access-token";
const REPLACEMENT_REFRESH_TOKEN: &str = "replacement-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const OMITTED_REFRESH_TOKEN_CHILD_SERVER_URL_ENV: &str =
    "MCP_TEST_OAUTH_OMITTED_REFRESH_TOKEN_SERVER_URL";
const LIVE_REFRESH_TOKEN_CHILD_SERVER_URL_ENV: &str =
    "MCP_TEST_OAUTH_LIVE_REFRESH_TOKEN_SERVER_URL";

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
async fn omitted_refresh_token_preserves_previous_for_next_refresh() -> anyhow::Result<()> {
    fn respond_to_initialize(request: &Request) -> ResponseTemplate {
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
                        "name": "oauth-omitted-refresh-token-test",
                        "version": "0.0.0-test",
                    },
                },
            })),
            Some("notifications/initialized") => ResponseTemplate::new(202),
            method => ResponseTemplate::new(400)
                .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
        }
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(2)
        .mount(&server)
        .await;
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let refresh_count_for_responder = Arc::clone(&refresh_count);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN}"
        )))
        .respond_with(move |_request: &Request| {
            match refresh_count_for_responder.fetch_add(1, Ordering::SeqCst) {
                0 => ResponseTemplate::new(200).set_body_json(json!({
                    "access_token": REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                })),
                1 => ResponseTemplate::new(200).set_body_json(json!({
                    "access_token": SECOND_REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": REPLACEMENT_REFRESH_TOKEN,
                })),
                request_count => ResponseTemplate::new(500)
                    .set_body_string(format!("unexpected refresh request {request_count}")),
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(respond_to_initialize)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {SECOND_REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(respond_to_initialize)
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_preserved_refresh_token_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(OMITTED_REFRESH_TOKEN_CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth preserved refresh token child failed: {status}"
    );
    assert_eq!(refresh_count.load(Ordering::SeqCst), 2);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by omitted_refresh_token_preserves_previous_for_next_refresh"]
async fn oauth_preserved_refresh_token_child() -> anyhow::Result<()> {
    let server_url = std::env::var(OMITTED_REFRESH_TOKEN_CHILD_SERVER_URL_ENV)?;
    let codex_home = std::env::var("CODEX_HOME")?;

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

    let first_client = RmcpClient::new_streamable_http_client(
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
    initialize_client(&first_client).await?;
    first_client.shutdown().await;

    let credentials_path = std::path::Path::new(&codex_home).join(".credentials.json");
    let persisted_after_first_refresh: Value =
        serde_json::from_str(&std::fs::read_to_string(&credentials_path)?)?;
    let persisted_entries = persisted_after_first_refresh
        .as_object()
        .expect("persisted OAuth credential map");
    assert_eq!(persisted_entries.len(), 1);
    let persisted_entry = persisted_entries
        .values()
        .next()
        .expect("one persisted OAuth credential entry");
    assert_eq!(
        persisted_entry.get("server_name"),
        Some(&json!(SERVER_NAME))
    );
    assert_eq!(persisted_entry.get("server_url"), Some(&json!(&server_url)));
    assert_eq!(
        persisted_entry.get("client_id"),
        Some(&json!("test-client-id"))
    );
    assert_eq!(
        persisted_entry.get("access_token"),
        Some(&json!(REFRESHED_ACCESS_TOKEN))
    );
    assert_eq!(
        persisted_entry.get("refresh_token"),
        Some(&json!(REFRESH_TOKEN))
    );
    assert_eq!(persisted_entry.get("scopes"), Some(&json!([])));
    assert!(
        persisted_entry
            .get("expires_at")
            .is_some_and(Value::is_number)
    );

    // Age the persisted access token without changing the refresh result.
    let mut expired_persisted_credentials = persisted_after_first_refresh.clone();
    let expired_entry = expired_persisted_credentials
        .as_object_mut()
        .and_then(|entries| entries.values_mut().next())
        .expect("one persisted OAuth credential entry");
    expired_entry["expires_at"] = json!(0);
    std::fs::write(
        &credentials_path,
        serde_json::to_string(&expired_persisted_credentials)?,
    )?;

    let second_client = RmcpClient::new_streamable_http_client(
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
    initialize_client(&second_client).await?;
    second_client.shutdown().await;

    let persisted_after_second_refresh: Value =
        serde_json::from_str(&std::fs::read_to_string(credentials_path)?)?;
    let persisted_entry = persisted_after_second_refresh
        .as_object()
        .and_then(|entries| entries.values().next())
        .expect("one persisted OAuth credential entry");
    assert_eq!(
        persisted_entry.get("access_token"),
        Some(&json!(SECOND_REFRESHED_ACCESS_TOKEN))
    );
    assert_eq!(
        persisted_entry.get("refresh_token"),
        Some(&json!(REPLACEMENT_REFRESH_TOKEN))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn omitted_refresh_token_preserves_running_manager_state() -> anyhow::Result<()> {
    fn respond_to_mcp(request: &Request) -> ResponseTemplate {
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
                        "name": "oauth-live-refresh-token-test",
                        "version": "0.0.0-test",
                    },
                },
            })),
            Some("notifications/initialized") => ResponseTemplate::new(202),
            Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": {
                    "tools": [],
                },
            })),
            method => ResponseTemplate::new(400)
                .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
        }
    }

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

    let refresh_count = Arc::new(AtomicUsize::new(0));
    let refresh_count_for_responder = Arc::clone(&refresh_count);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(move |request: &Request| {
            let body = String::from_utf8_lossy(&request.body);
            match refresh_count_for_responder.fetch_add(1, Ordering::SeqCst) {
                0 if body.contains(&format!("refresh_token={REFRESH_TOKEN}")) => {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "access_token": REFRESHED_ACCESS_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 0,
                    }))
                }
                1 if body.contains(&format!("refresh_token={REFRESH_TOKEN}")) => {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "access_token": SECOND_REFRESHED_ACCESS_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 0,
                        "refresh_token": REPLACEMENT_REFRESH_TOKEN,
                    }))
                }
                2 if body.contains(&format!("refresh_token={REPLACEMENT_REFRESH_TOKEN}")) => {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "access_token": THIRD_REFRESHED_ACCESS_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 7200,
                    }))
                }
                request_count => ResponseTemplate::new(400).set_body_string(format!(
                    "unexpected refresh request {request_count}: {body}"
                )),
            }
        })
        .expect(3)
        .mount(&server)
        .await;

    for access_token in [
        REFRESHED_ACCESS_TOKEN,
        SECOND_REFRESHED_ACCESS_TOKEN,
        THIRD_REFRESHED_ACCESS_TOKEN,
    ] {
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(header("authorization", format!("Bearer {access_token}")))
            .respond_with(respond_to_mcp)
            .expect(1)
            .mount(&server)
            .await;
    }

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_live_refresh_token_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(LIVE_REFRESH_TOKEN_CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth live refresh token child failed: {status}"
    );
    assert_eq!(refresh_count.load(Ordering::SeqCst), 3);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by omitted_refresh_token_preserves_running_manager_state"]
async fn oauth_live_refresh_token_child() -> anyhow::Result<()> {
    let server_url = std::env::var(LIVE_REFRESH_TOKEN_CHILD_SERVER_URL_ENV)?;
    let codex_home = std::env::var("CODEX_HOME")?;

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

    let credentials_path = std::path::Path::new(&codex_home).join(".credentials.json");
    let persisted_after_replacement: Value =
        serde_json::from_str(&std::fs::read_to_string(&credentials_path)?)?;
    let persisted_entry = persisted_after_replacement
        .as_object()
        .and_then(|entries| entries.values().next())
        .expect("one persisted OAuth credential entry");
    assert_eq!(
        persisted_entry.get("access_token"),
        Some(&json!(SECOND_REFRESHED_ACCESS_TOKEN))
    );
    assert_eq!(
        persisted_entry.get("refresh_token"),
        Some(&json!(REPLACEMENT_REFRESH_TOKEN))
    );

    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    assert!(tools.tools.is_empty());

    let persisted_after_third_refresh: Value =
        serde_json::from_str(&std::fs::read_to_string(credentials_path)?)?;
    let persisted_entry = persisted_after_third_refresh
        .as_object()
        .and_then(|entries| entries.values().next())
        .expect("one persisted OAuth credential entry");
    assert_eq!(
        persisted_entry.get("access_token"),
        Some(&json!(THIRD_REFRESHED_ACCESS_TOKEN))
    );
    assert_eq!(
        persisted_entry.get("refresh_token"),
        Some(&json!(REPLACEMENT_REFRESH_TOKEN))
    );

    client.shutdown().await;
    Ok(())
}
