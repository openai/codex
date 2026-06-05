mod streamable_http_test_support;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR;
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
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SCENARIO";
const CREDENTIALS_FILENAME: &str = ".credentials.json";
const SCENARIO_SUCCESS: &str = "success";
const SCENARIO_INVALID_GRANT: &str = "invalid_grant";
const SCENARIO_TRANSIENT_FAILURE: &str = "transient_failure";
const SCENARIO_LOAD_FAILURE: &str = "load_failure";
const SCENARIO_PERSIST_FAILURE: &str = "persist_failure";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server, /*expected_calls*/ 1).await;
    mount_successful_mcp_requests(&server, /*expected_calls*/ 2).await;

    run_oauth_startup_child(&server, SCENARIO_SUCCESS).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invalid_grant_stops_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "refresh token revoked",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    run_oauth_startup_child(&server, SCENARIO_INVALID_GRANT).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn transient_refresh_failure_stops_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(ResponseTemplate::new(503).set_body_string("temporarily unavailable"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    run_oauth_startup_child(&server, SCENARIO_TRANSIENT_FAILURE).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn credential_load_failure_stops_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    run_oauth_startup_child(&server, SCENARIO_LOAD_FAILURE).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn credential_persist_failure_is_swallowed_after_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    mount_successful_refresh(&server, /*expected_calls*/ 1).await;
    mount_successful_mcp_requests(&server, /*expected_calls*/ 3).await;

    run_oauth_startup_child(&server, SCENARIO_PERSIST_FAILURE).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by OAuth startup integration tests"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let scenario = std::env::var(CHILD_SCENARIO_ENV)?;
    let credentials_path = credentials_path()?;

    if scenario == SCENARIO_LOAD_FAILURE {
        fs::create_dir(&credentials_path)?;
    } else {
        save_expired_tokens(&server_url)?;
    }

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
    .await;

    if matches!(
        scenario.as_str(),
        SCENARIO_INVALID_GRANT | SCENARIO_TRANSIENT_FAILURE | SCENARIO_LOAD_FAILURE
    ) {
        let error = match client {
            Ok(_) => anyhow::bail!("OAuth startup should fail for scenario {scenario}"),
            Err(error) => format!("{error:#}"),
        };
        match scenario.as_str() {
            SCENARIO_INVALID_GRANT => {
                assert!(
                    error.contains(OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR),
                    "unexpected invalid_grant failure: {error}"
                );
                assert!(
                    !error.contains("invalid_grant") && !error.contains("refresh token revoked"),
                    "revoked-token provider details should be hidden: {error}"
                );
            }
            SCENARIO_TRANSIENT_FAILURE => {
                assert!(
                    error.contains("OAuth token endpoint refresh failed"),
                    "unexpected transient refresh failure: {error}"
                );
                assert!(
                    !error.contains(OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR),
                    "transient refresh failure should not request reauthentication: {error}"
                );
            }
            SCENARIO_LOAD_FAILURE => {
                assert!(
                    error.contains("failed to read OAuth credentials for MCP server"),
                    "unexpected credential load failure: {error}"
                );
                assert!(
                    error.contains("failed to read credentials file"),
                    "credential storage cause should be preserved: {error}"
                );
                assert!(
                    !error.contains("Auth required"),
                    "credential storage failure should not be classified as unauthenticated: {error}"
                );
            }
            _ => unreachable!(),
        }
        return Ok(());
    }
    let client = client?;

    let original_permissions = fs::metadata(&credentials_path)?.permissions();
    if scenario == SCENARIO_PERSIST_FAILURE {
        let mut readonly_permissions = original_permissions.clone();
        readonly_permissions.set_readonly(true);
        fs::set_permissions(&credentials_path, readonly_permissions)?;
        let stale_credentials = fs::read_to_string(&credentials_path)?;
        let write_error = fs::write(&credentials_path, &stale_credentials)
            .expect_err("read-only credentials file should reject writes");
        assert_eq!(write_error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    let result = initialize_client(&client).await;
    match scenario.as_str() {
        SCENARIO_SUCCESS => result?,
        SCENARIO_PERSIST_FAILURE => {
            result?;
            client
                .list_tools(
                    /*params*/ None,
                    /*timeout*/ Some(Duration::from_secs(5)),
                )
                .await?;
            fs::set_permissions(&credentials_path, original_permissions)?;
            assert!(
                credentials_path.is_file(),
                "failed persistence should leave the credentials file in place"
            );
            let stale_credentials = fs::read_to_string(&credentials_path)?;
            assert!(
                stale_credentials.contains(EXPIRED_ACCESS_TOKEN),
                "the only persisted credentials should remain stale"
            );
            assert!(
                !stale_credentials.contains(REFRESHED_ACCESS_TOKEN),
                "refreshed credentials should not have been persisted"
            );
        }
        SCENARIO_INVALID_GRANT | SCENARIO_TRANSIENT_FAILURE | SCENARIO_LOAD_FAILURE => {
            unreachable!()
        }
        other => anyhow::bail!("unknown OAuth startup scenario: {other}"),
    }
    Ok(())
}

async fn run_oauth_startup_child(server: &MockServer, scenario: &str) -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());

    // Credential storage resolves CODEX_HOME from the process environment.
    // Run the client half of the test in an ignored helper test so it can use
    // an isolated home without mutating the parent test runner's environment.
    let status = Command::new(std::env::current_exe()?)
        .args(["oauth_startup_child", "--exact", "--ignored", "--nocapture"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .env(CHILD_SCENARIO_ENV, scenario)
        .status()
        .await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
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

async fn mount_successful_refresh(server: &MockServer, expected_calls: u64) {
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
        .expect(expected_calls)
        .mount(server)
        .await;
}

async fn mount_successful_mcp_requests(server: &MockServer, expected_calls: u64) {
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
        })
        .expect(expected_calls)
        .mount(server)
        .await;
}

fn save_expired_tokens(server_url: &str) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
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
        expires_at: Some(0),
    };
    save_oauth_tokens(SERVER_NAME, &tokens, OAuthCredentialsStoreMode::File)
}

fn credentials_path() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("CODEX_HOME")?).join(CREDENTIALS_FILENAME))
}
