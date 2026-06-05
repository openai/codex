mod streamable_http_test_support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
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
use serde::Deserialize;
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
const STARTUP_REFRESH_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_REFRESH_SERVER_URL";
const EXTERNAL_UPDATE_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_EXTERNAL_UPDATE_SERVER_URL";
const OPERATION_TIMEOUT_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_TIMEOUT_SERVER_URL";
const OPERATION_TIMEOUT_REFRESH_STARTED_ENV: &str = "MCP_TEST_OAUTH_TIMEOUT_REFRESH_STARTED";
const PERSIST_TIMEOUT_CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_PERSIST_TIMEOUT_SERVER_URL";
const PERSIST_TIMEOUT_OPERATION_STARTED_ENV: &str =
    "MCP_TEST_OAUTH_PERSIST_TIMEOUT_OPERATION_STARTED";
const PERSIST_TIMEOUT_REFRESH_STARTED_ENV: &str = "MCP_TEST_OAUTH_PERSIST_TIMEOUT_REFRESH_STARTED";
const REFRESH_CONTENTION_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_SERVER_URL";
const REFRESH_CONTENTION_ATTEMPT_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_ATTEMPT";
const REFRESH_CONTENTION_DONE_ENV: &str = "MCP_TEST_OAUTH_REFRESH_CONTENTION_DONE";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const VALID_REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const OLD_ACCESS_TOKEN: &str = "old-access-token";
const OLD_REFRESH_TOKEN: &str = "old-refresh-token";
const EXTERNAL_ACCESS_TOKEN: &str = "external-access-token";
const EXTERNAL_REFRESH_TOKEN: &str = "external-refresh-token";
const EXTERNAL_EXPIRES_AT: u64 = u64::MAX;
const TIMEOUT_REFRESHED_ACCESS_TOKEN: &str = "timeout-refreshed-access-token";
const STALE_REFRESHED_ACCESS_TOKEN: &str = "stale-refreshed-access-token";
const STALE_ROTATED_REFRESH_TOKEN: &str = "stale-rotated-refresh-token";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_calls*/ 1).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={VALID_REFRESH_TOKEN}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": REFRESHED_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": VALID_REFRESH_TOKEN,
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
        .respond_with(|request: &Request| oauth_mcp_response(request, REFRESHED_ACCESS_TOKEN))
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_startup_refresh_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(STARTUP_REFRESH_CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refreshes_expired_persisted_token_before_initialize"]
async fn oauth_startup_refresh_child() -> anyhow::Result<()> {
    let server_url = std::env::var(STARTUP_REFRESH_CHILD_SERVER_URL_ENV)?;
    save_test_tokens(
        &server_url,
        EXPIRED_ACCESS_TOKEN,
        VALID_REFRESH_TOKEN,
        /*expires_at*/ 0,
    )?;
    initialized_oauth_client(&server_url).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn operation_timeout_includes_oauth_generation_lock_wait() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_calls*/ 3).await;
    let codex_home = TempDir::new()?;
    let refresh_started = codex_home.path().join("refresh-started");
    let refresh_started_for_mock = refresh_started.clone();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={EXTERNAL_REFRESH_TOKEN}"
        )))
        .respond_with(move |_request: &Request| {
            std::fs::write(&refresh_started_for_mock, b"started").expect("write refresh marker");
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(1))
                .set_body_json(json!({
                    "access_token": TIMEOUT_REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": EXTERNAL_REFRESH_TOKEN,
                }))
        })
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let Ok(body) = request.body_json::<Value>() else {
                return ResponseTemplate::new(400);
            };
            let expected_access_token = match (
                body.get("method").and_then(Value::as_str),
                body.pointer("/params/name").and_then(Value::as_str),
            ) {
                (Some("tools/call"), Some("hold-generation-lock")) => {
                    TIMEOUT_REFRESHED_ACCESS_TOKEN
                }
                _ => OLD_ACCESS_TOKEN,
            };
            oauth_mcp_response(request, expected_access_token)
        })
        .expect(5)
        .mount(&server)
        .await;

    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_operation_timeout_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(OPERATION_TIMEOUT_CHILD_SERVER_URL_ENV, server_url)
        .env(OPERATION_TIMEOUT_REFRESH_STARTED_ENV, refresh_started)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth operation timeout child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by operation_timeout_includes_oauth_generation_lock_wait"]
async fn oauth_operation_timeout_child() -> anyhow::Result<()> {
    let server_url = std::env::var(OPERATION_TIMEOUT_CHILD_SERVER_URL_ENV)?;
    let refresh_started =
        std::path::PathBuf::from(std::env::var(OPERATION_TIMEOUT_REFRESH_STARTED_ENV)?);
    save_test_tokens(
        &server_url,
        OLD_ACCESS_TOKEN,
        OLD_REFRESH_TOKEN,
        future_expiry()?,
    )?;
    let lock_holder = Arc::new(initialized_oauth_client(&server_url).await?);
    let timed_client = initialized_oauth_client(&server_url).await?;
    save_test_tokens(
        &server_url,
        EXTERNAL_ACCESS_TOKEN,
        EXTERNAL_REFRESH_TOKEN,
        /*expires_at*/ 0,
    )?;

    let lock_holder_call = tokio::spawn(async move {
        lock_holder
            .call_tool(
                "hold-generation-lock".to_string(),
                /*arguments*/ None,
                /*meta*/ None,
                Some(Duration::from_secs(5)),
            )
            .await
    });
    wait_for_path(&refresh_started).await?;
    let error = timed_client
        .call_tool(
            "must-time-out-on-generation-lock".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_millis(100)),
        )
        .await
        .expect_err("generation lock wait must consume the operation timeout");
    assert!(
        error
            .to_string()
            .contains("timed out awaiting tools/call after 100ms"),
        "unexpected timeout error: {error}"
    );
    lock_holder_call.await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn operation_timeout_includes_post_operation_persistence_lock_wait() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_calls*/ 3).await;
    let codex_home = TempDir::new()?;
    let operation_started = codex_home.path().join("operation-started");
    let refresh_started = codex_home.path().join("refresh-started");
    let refresh_started_for_mock = refresh_started.clone();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={EXTERNAL_REFRESH_TOKEN}"
        )))
        .respond_with(move |_request: &Request| {
            std::fs::write(&refresh_started_for_mock, b"started").expect("write refresh marker");
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(json!({
                    "access_token": TIMEOUT_REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": EXTERNAL_REFRESH_TOKEN,
                }))
        })
        .expect(1)
        .mount(&server)
        .await;
    let operation_started_for_mock = operation_started.clone();
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let Ok(body) = request.body_json::<Value>() else {
                return ResponseTemplate::new(400);
            };
            let request_name = body.pointer("/params/name").and_then(Value::as_str);
            let expected_access_token = match request_name {
                Some("hold-generation-lock-during-persistence") => TIMEOUT_REFRESHED_ACCESS_TOKEN,
                _ => OLD_ACCESS_TOKEN,
            };
            let response = oauth_mcp_response(request, expected_access_token);
            if request_name == Some("successful-call-before-persist-contention") {
                std::fs::write(&operation_started_for_mock, b"started")
                    .expect("write operation marker");
                response.set_delay(Duration::from_secs(1))
            } else {
                response
            }
        })
        .expect(6)
        .mount(&server)
        .await;

    let server_url = format!("{}/mcp", server.uri());
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_persist_timeout_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(PERSIST_TIMEOUT_CHILD_SERVER_URL_ENV, server_url)
        .env(PERSIST_TIMEOUT_OPERATION_STARTED_ENV, operation_started)
        .env(PERSIST_TIMEOUT_REFRESH_STARTED_ENV, refresh_started)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth persistence timeout child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by operation_timeout_includes_post_operation_persistence_lock_wait"]
async fn oauth_persist_timeout_child() -> anyhow::Result<()> {
    let server_url = std::env::var(PERSIST_TIMEOUT_CHILD_SERVER_URL_ENV)?;
    let operation_started =
        std::path::PathBuf::from(std::env::var(PERSIST_TIMEOUT_OPERATION_STARTED_ENV)?);
    let refresh_started =
        std::path::PathBuf::from(std::env::var(PERSIST_TIMEOUT_REFRESH_STARTED_ENV)?);
    save_test_tokens(
        &server_url,
        OLD_ACCESS_TOKEN,
        OLD_REFRESH_TOKEN,
        future_expiry()?,
    )?;
    let timed_client = Arc::new(initialized_oauth_client(&server_url).await?);
    let lock_holder = Arc::new(initialized_oauth_client(&server_url).await?);

    let timed_call = tokio::spawn(async move {
        timed_client
            .call_tool(
                "successful-call-before-persist-contention".to_string(),
                /*arguments*/ None,
                /*meta*/ None,
                Some(Duration::from_millis(1500)),
            )
            .await
    });
    wait_for_path(&operation_started).await?;
    save_test_tokens(
        &server_url,
        EXTERNAL_ACCESS_TOKEN,
        EXTERNAL_REFRESH_TOKEN,
        /*expires_at*/ 0,
    )?;
    let lock_holder_call = tokio::spawn(async move {
        lock_holder
            .call_tool(
                "hold-generation-lock-during-persistence".to_string(),
                /*arguments*/ None,
                /*meta*/ None,
                Some(Duration::from_secs(5)),
            )
            .await
    });
    wait_for_path(&refresh_started).await?;

    let error = timed_call
        .await?
        .expect_err("persistence lock wait must consume the operation timeout");
    assert!(
        error.to_string().contains("timed out awaiting tools/call"),
        "unexpected timeout error: {error}"
    );
    lock_holder_call.await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn running_client_adopts_external_oauth_update_and_deletion() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_calls*/ 2).await;

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
        EXTERNAL_EXPIRES_AT,
    )?;

    client
        .call_tool(
            "call-after-external-update".to_string(),
            /*arguments*/ None,
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;

    assert_persisted_tokens(&server_url, EXTERNAL_ACCESS_TOKEN, EXTERNAL_REFRESH_TOKEN)?;
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
    mount_oauth_metadata(&server, /*expected_calls*/ 3).await;
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

    assert_persisted_tokens(&server_url, EXTERNAL_ACCESS_TOKEN, EXTERNAL_REFRESH_TOKEN)?;
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
        EXTERNAL_EXPIRES_AT,
    )?;
    std::fs::write(done_path, b"done")?;
    Ok(())
}

async fn mount_oauth_metadata(server: &MockServer, expected_calls: u64) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(expected_calls)
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

#[derive(Debug, Deserialize, PartialEq)]
struct PersistedCredentialEntry {
    server_name: String,
    server_url: String,
    client_id: String,
    access_token: String,
    expires_at: Option<u64>,
    refresh_token: Option<String>,
    scopes: Vec<String>,
}

fn assert_persisted_tokens(
    server_url: &str,
    access_token: &str,
    refresh_token: &str,
) -> anyhow::Result<()> {
    let credentials = std::fs::read_to_string(
        std::path::Path::new(&std::env::var("CODEX_HOME")?).join(".credentials.json"),
    )?;
    let store = serde_json::from_str::<BTreeMap<String, PersistedCredentialEntry>>(&credentials)?;
    let actual = store
        .values()
        .find(|entry| entry.server_name == SERVER_NAME && entry.server_url == server_url)
        .context("target persisted OAuth credential entry should exist")?;
    assert_eq!(
        actual,
        &PersistedCredentialEntry {
            server_name: SERVER_NAME.to_string(),
            server_url: server_url.to_string(),
            client_id: "test-client-id".to_string(),
            access_token: access_token.to_string(),
            expires_at: Some(EXTERNAL_EXPIRES_AT),
            refresh_token: Some(refresh_token.to_string()),
            scopes: Vec::new(),
        }
    );
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
