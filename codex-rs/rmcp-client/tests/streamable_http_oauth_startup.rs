mod streamable_http_test_support;

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::delete_oauth_tokens;
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
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_EXTERNAL_UPDATE_SERVER_URL";
const REFRESH_CONTENTION_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_SERVER_URL";
const REFRESH_CONTENTION_ATTEMPT_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_ATTEMPT";
const REFRESH_CONTENTION_DONE_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_DONE";
const OLD_ACCESS_TOKEN: &str = "old-access-token";
const OLD_REFRESH_TOKEN: &str = "old-refresh-token";
const EXTERNAL_ACCESS_TOKEN: &str = "external-access-token";
const EXTERNAL_REFRESH_TOKEN: &str = "external-refresh-token";
const STALE_REFRESHED_ACCESS_TOKEN: &str = "stale-refreshed-access-token";
const STALE_ROTATED_REFRESH_TOKEN: &str = "stale-rotated-refresh-token";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn running_client_adopts_external_oauth_update_and_deletion() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let Ok(body) = request.body_json::<Value>() else {
                return ResponseTemplate::new(400);
            };
            let request_method = body.get("method").and_then(Value::as_str);
            let expected_access_token = match request_method {
                Some("tools/call") => EXTERNAL_ACCESS_TOKEN,
                _ => OLD_ACCESS_TOKEN,
            };
            oauth_mcp_response(request, expected_access_token)
        })
        .expect(3)
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
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by running_client_adopts_external_oauth_update_and_deletion"]
async fn oauth_external_update_child() -> anyhow::Result<()> {
    let server_url = std::env::var(EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV)?;
    save_test_tokens(
        &server_url,
        OLD_ACCESS_TOKEN,
        OLD_REFRESH_TOKEN,
        future_expiry()?,
    )?;

    let client = initialized_oauth_client(&server_url).await?;

    save_test_tokens(
        &server_url,
        EXTERNAL_ACCESS_TOKEN,
        EXTERNAL_REFRESH_TOKEN,
        future_expiry()?,
    )?;

    client
        .call_tool(
            "call-after-external-update".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    assert_persisted_tokens(EXTERNAL_ACCESS_TOKEN, EXTERNAL_REFRESH_TOKEN)?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_external_delete_writer_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", std::env::var("CODEX_HOME")?)
        .env(EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV, &server_url)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth external delete writer failed: {status}"
    );

    client
        .call_tool(
            "call-after-external-delete".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await
        .expect_err("externally deleted credentials must not be reused");
    assert!(
        !std::path::Path::new(&std::env::var("CODEX_HOME")?)
            .join(".credentials.json")
            .exists()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by oauth_external_update_child"]
async fn oauth_external_delete_writer_child() -> anyhow::Result<()> {
    let server_url = std::env::var(EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV)?;
    assert!(delete_oauth_tokens(
        SERVER_NAME,
        &server_url,
        OAuthCredentialsStoreMode::File,
    )?);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_holds_generation_lock_against_external_oauth_update() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let server_url = format!("{}/mcp", server.uri());
    mount_oauth_metadata(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={OLD_REFRESH_TOKEN}"
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(1500))
                .set_body_json(json!({
                    "access_token": STALE_REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": STALE_ROTATED_REFRESH_TOKEN,
                })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let Ok(body) = request.body_json::<Value>() else {
                return ResponseTemplate::new(400);
            };
            let request_method = body.get("method").and_then(Value::as_str);
            let expected_access_token = match (
                request_method,
                body.pointer("/params/name").and_then(Value::as_str),
            ) {
                (Some("initialize" | "notifications/initialized"), _) => OLD_ACCESS_TOKEN,
                (Some("tools/call"), Some("trigger-refresh-contention")) => {
                    STALE_REFRESHED_ACCESS_TOKEN
                }
                (Some("tools/call"), Some("call-after-refresh-contention")) => {
                    EXTERNAL_ACCESS_TOKEN
                }
                _ => OLD_ACCESS_TOKEN,
            };
            oauth_mcp_response(request, expected_access_token)
        })
        .expect(4)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let attempt_path = codex_home.path().join("external-update-attempt");
    let done_path = codex_home.path().join("external-update-done");
    let mut live_client = Command::new(std::env::current_exe()?)
        .args([
            "oauth_external_update_during_refresh_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(REFRESH_CONTENTION_SERVER_URL_ENV, &server_url)
        .env(REFRESH_CONTENTION_DONE_ENV, &done_path)
        .spawn()?;

    wait_for_request_path(&server, "/oauth/token").await?;
    let mut external_writer = Command::new(std::env::current_exe()?)
        .args([
            "oauth_external_update_writer_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(REFRESH_CONTENTION_SERVER_URL_ENV, &server_url)
        .env(REFRESH_CONTENTION_ATTEMPT_ENV, &attempt_path)
        .env(REFRESH_CONTENTION_DONE_ENV, &done_path)
        .spawn()?;

    wait_for_path(&attempt_path).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !done_path.exists(),
        "external writer should block while refresh owns the generation lock"
    );
    assert!(
        external_writer.try_wait()?.is_none(),
        "external writer exited before the refresh released its generation lock"
    );

    let writer_status = external_writer.wait().await?;
    assert!(
        writer_status.success(),
        "OAuth external update writer failed: {writer_status}"
    );
    let live_status = live_client.wait().await?;
    assert!(
        live_status.success(),
        "OAuth refresh contention child failed: {live_status}"
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refresh_holds_generation_lock_against_external_oauth_update"]
async fn oauth_external_update_during_refresh_child() -> anyhow::Result<()> {
    let server_url = std::env::var(REFRESH_CONTENTION_SERVER_URL_ENV)?;
    let done_path = std::path::PathBuf::from(std::env::var(REFRESH_CONTENTION_DONE_ENV)?);
    save_test_tokens(
        &server_url,
        OLD_ACCESS_TOKEN,
        OLD_REFRESH_TOKEN,
        future_expiry()?,
    )?;

    let client = initialized_oauth_client(&server_url).await?;
    save_test_tokens(&server_url, OLD_ACCESS_TOKEN, OLD_REFRESH_TOKEN, 0)?;
    client
        .call_tool(
            "trigger-refresh-contention".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;
    wait_for_path(&done_path).await?;
    client
        .call_tool(
            "call-after-refresh-contention".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    assert_persisted_tokens(EXTERNAL_ACCESS_TOKEN, EXTERNAL_REFRESH_TOKEN)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refresh_holds_generation_lock_against_external_oauth_update"]
async fn oauth_external_update_writer_child() -> anyhow::Result<()> {
    let server_url = std::env::var(REFRESH_CONTENTION_SERVER_URL_ENV)?;
    let attempt_path = std::env::var(REFRESH_CONTENTION_ATTEMPT_ENV)?;
    let done_path = std::env::var(REFRESH_CONTENTION_DONE_ENV)?;
    std::fs::write(attempt_path, b"attempting")?;
    save_test_tokens(
        &server_url,
        EXTERNAL_ACCESS_TOKEN,
        EXTERNAL_REFRESH_TOKEN,
        future_expiry()?,
    )?;
    std::fs::write(done_path, b"done")?;
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

async fn initialized_oauth_client(server_url: &str) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    initialize_client(&client).await?;
    Ok(client)
}

fn oauth_mcp_response(request: &Request, expected_access_token: &str) -> ResponseTemplate {
    let authorization = request
        .headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if authorization != Some(format!("Bearer {expected_access_token}").as_str()) {
        return ResponseTemplate::new(401);
    }

    let Ok(body) = request.body_json::<Value>() else {
        return ResponseTemplate::new(400);
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
                "capabilities": {
                    "tools": {},
                },
                "serverInfo": {
                    "name": "oauth-external-credential-test",
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

async fn wait_for_request_path(server: &MockServer, expected_path: &str) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if server
                .received_requests()
                .await
                .unwrap_or_default()
                .iter()
                .any(|request| request.url.path() == expected_path)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

async fn wait_for_path(path: &std::path::Path) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    Ok(())
}

fn future_expiry() -> anyhow::Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .saturating_add(Duration::from_secs(7200))
        .as_millis() as u64)
}

fn assert_persisted_tokens(access_token: &str, refresh_token: &str) -> anyhow::Result<()> {
    let credentials = std::fs::read_to_string(
        std::path::Path::new(&std::env::var("CODEX_HOME")?).join(".credentials.json"),
    )?;
    assert!(credentials.contains(access_token));
    assert!(credentials.contains(refresh_token));
    assert!(!credentials.contains(STALE_REFRESHED_ACCESS_TOKEN));
    assert!(!credentials.contains(STALE_ROTATED_REFRESH_TOKEN));
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
