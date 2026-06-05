mod streamable_http_test_support;

use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
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
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::sleep;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-refresh-race";
const CLIENT_ID: &str = "test-client-id";
const OLD_ACCESS_TOKEN: &str = "old-access-token";
const OLD_REFRESH_TOKEN: &str = "old-one-time-refresh-token";
const NEW_ACCESS_TOKEN: &str = "new-access-token";
const NEW_REFRESH_TOKEN: &str = "new-one-time-refresh-token";
const KEYRING_SERVICE: &str = "Codex MCP Credentials";
const CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_millis(100);

const SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_RACE_SERVER_URL";
const READY_PATH_ENV: &str = "MCP_TEST_OAUTH_RACE_READY_PATH";
const GO_PATH_ENV: &str = "MCP_TEST_OAUTH_RACE_GO_PATH";
const RESULT_PATH_ENV: &str = "MCP_TEST_OAUTH_RACE_RESULT_PATH";
const ACCESS_TOKEN_ENV: &str = "MCP_TEST_OAUTH_RACE_ACCESS_TOKEN";
const REFRESH_TOKEN_ENV: &str = "MCP_TEST_OAUTH_RACE_REFRESH_TOKEN";
const EXPIRES_AT_ENV: &str = "MCP_TEST_OAUTH_RACE_EXPIRES_AT";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrent_processes_coordinate_one_time_refresh_and_reload_rotated_tokens()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 2).await;

    let refresh_attempts = Arc::new(AtomicUsize::new(0));
    let responder_attempts = Arc::clone(&refresh_attempts);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(move |_request: &Request| {
            if responder_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(1))
                    .set_body_json(json!({
                        "access_token": NEW_ACCESS_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 7200,
                        "refresh_token": NEW_REFRESH_TOKEN,
                    }))
            } else {
                ResponseTemplate::new(400).set_body_json(json!({
                    "error": "invalid_grant",
                    "error_description": "refresh token already used",
                }))
            }
        })
        .expect(1)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 4).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let go_paths = [
        control_dir.path().join("client-a.go"),
        control_dir.path().join("client-b.go"),
    ];
    let ready_paths = [
        control_dir.path().join("client-a.ready"),
        control_dir.path().join("client-b.ready"),
    ];
    let result_paths = [
        control_dir.path().join("client-a.result"),
        control_dir.path().join("client-b.result"),
    ];
    let mut children = Vec::new();
    for ((ready_path, go_path), result_path) in ready_paths
        .iter()
        .zip(go_paths.iter())
        .zip(result_paths.iter())
    {
        children.push(spawn_client_child(
            &codex_home,
            &server_url,
            ready_path,
            go_path,
            result_path,
        )?);
    }

    wait_for_paths(&ready_paths).await?;
    fs::write(&go_paths[0], "go")?;
    timeout(Duration::from_secs(5), async {
        while refresh_attempts.load(Ordering::SeqCst) == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for the first process to hold the refresh lock")?;
    fs::write(&go_paths[1], "go")?;
    wait_for_children(children).await?;

    let outcomes = read_outcomes(&result_paths)?;
    assert_eq!(outcomes, vec!["success".to_string(), "success".to_string()]);
    assert_eq!(refresh_attempts.load(Ordering::SeqCst), 1);

    let requests = server
        .received_requests()
        .await
        .context("wiremock request recording disabled")?;
    let refresh_bodies = request_bodies(&requests, "/oauth/token");
    assert_eq!(refresh_bodies.len(), 1);
    assert!(
        refresh_bodies
            .iter()
            .all(|body| body.contains(&format!("refresh_token={OLD_REFRESH_TOKEN}")))
    );
    let authorization_headers = requests
        .iter()
        .filter(|request| request.method.as_str() == "POST" && request.url.path() == "/mcp")
        .map(|request| {
            request
                .headers
                .get("authorization")
                .expect("authorization header")
                .to_str()
                .expect("ASCII authorization header")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        authorization_headers,
        vec![format!("Bearer {NEW_ACCESS_TOKEN}"); 4]
    );
    assert_eq!(
        persisted_token_snapshot(&codex_home)?,
        PersistedTokenSnapshot {
            access_token: NEW_ACCESS_TOKEN.to_string(),
            refresh_token: Some(NEW_REFRESH_TOKEN.to_string()),
        }
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_lock_wait_uses_startup_timeout_budget() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 1).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let lock_path = file_oauth_refresh_lock_path(&codex_home, &server_url)?;
    let refresh_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    refresh_lock.lock()?;

    let ready_path = control_dir.path().join("client.ready");
    let go_path = control_dir.path().join("client.go");
    let result_path = control_dir.path().join("client.result");
    let child = spawn_client_child(
        &codex_home,
        &server_url,
        &ready_path,
        &go_path,
        &result_path,
    )?;
    wait_for_paths(std::slice::from_ref(&ready_path)).await?;
    fs::write(&go_path, "go")?;
    timeout(Duration::from_secs(7), wait_for_children(vec![child]))
        .await
        .context("startup did not time out while waiting for the refresh lock")??;

    assert_eq!(
        fs::read_to_string(result_path)?,
        "error:timed out handshaking with MCP server after 5s"
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_lock_wait_uses_operation_timeout_budget() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 1).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": NEW_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 30,
            "refresh_token": NEW_REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 2).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let ready_path = control_dir.path().join("client.ready");
    let go_path = control_dir.path().join("client.go");
    let result_path = control_dir.path().join("client.result");
    let child = spawn_operation_timeout_child(
        &codex_home,
        &server_url,
        &ready_path,
        &go_path,
        &result_path,
    )?;
    wait_for_paths(std::slice::from_ref(&ready_path)).await?;

    let lock_path = file_oauth_refresh_lock_path(&codex_home, &server_url)?;
    let refresh_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    refresh_lock.lock()?;

    fs::write(&go_path, "go")?;
    wait_for_children(vec![child]).await?;
    assert_eq!(
        fs::read_to_string(result_path)?,
        format!("error:timed out awaiting tools/list after {OPERATION_TIMEOUT:?}")
    );

    drop(refresh_lock);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_provider_call_uses_operation_timeout_budget() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 1).await;
    let refresh_attempts = Arc::new(AtomicUsize::new(0));
    let responder_attempts = Arc::clone(&refresh_attempts);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(move |_request: &Request| {
            let response = ResponseTemplate::new(200).set_body_json(json!({
                "access_token": NEW_ACCESS_TOKEN,
                "token_type": "Bearer",
                "expires_in": 30,
                "refresh_token": NEW_REFRESH_TOKEN,
            }));
            if responder_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                response
            } else {
                response.set_delay(Duration::from_secs(1))
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 2).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let ready_path = control_dir.path().join("client.ready");
    let go_path = control_dir.path().join("client.go");
    let result_path = control_dir.path().join("client.result");
    let child = spawn_operation_timeout_child(
        &codex_home,
        &server_url,
        &ready_path,
        &go_path,
        &result_path,
    )?;
    wait_for_paths(std::slice::from_ref(&ready_path)).await?;
    fs::write(&go_path, "go")?;
    wait_for_children(vec![child]).await?;

    assert_eq!(
        fs::read_to_string(result_path)?,
        format!("error:timed out awaiting tools/list after {OPERATION_TIMEOUT:?}")
    );
    assert_eq!(refresh_attempts.load(Ordering::SeqCst), 2);

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn distinct_file_credentials_do_not_block_each_other() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 1).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": NEW_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": NEW_REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 2).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let unrelated_lock_path =
        file_oauth_refresh_lock_path(&codex_home, "https://unrelated.example.test/mcp")?;
    let unrelated_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(unrelated_lock_path)?;
    unrelated_lock.lock()?;

    let ready_path = control_dir.path().join("client.ready");
    let go_path = control_dir.path().join("client.go");
    let result_path = control_dir.path().join("client.result");
    let child = spawn_client_child(
        &codex_home,
        &server_url,
        &ready_path,
        &go_path,
        &result_path,
    )?;
    wait_for_paths(std::slice::from_ref(&ready_path)).await?;
    fs::write(&go_path, "go")?;
    wait_for_children(vec![child]).await?;

    assert_eq!(fs::read_to_string(result_path)?, "success");
    drop(unrelated_lock);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejected_refresh_requires_login_without_reaching_mcp() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 1).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "refresh token revoked",
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 0).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;

    let ready_path = control_dir.path().join("client.ready");
    let go_path = control_dir.path().join("client.go");
    let result_path = control_dir.path().join("client.result");
    let child = spawn_client_child(
        &codex_home,
        &server_url,
        &ready_path,
        &go_path,
        &result_path,
    )?;
    wait_for_paths(std::slice::from_ref(&ready_path)).await?;
    fs::write(&go_path, "go")?;
    wait_for_children(vec![child]).await?;

    assert_eq!(
        fs::read_to_string(result_path)?,
        "error:Auth required: OAuth refresh token was rejected; reauthentication required"
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn process_keeps_stale_in_memory_tokens_but_does_not_overwrite_rotated_tokens()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    mount_oauth_metadata(&server, /*expected_requests*/ 2).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": NEW_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": NEW_REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_mcp_server(&server, /*expected_requests*/ 4).await;

    let codex_home = TempDir::new()?;
    let control_dir = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    let future_expiry = unix_millis().saturating_add(3_600_000);
    seed_tokens(&codex_home, &server_url, future_expiry).await?;

    let stale_ready = control_dir.path().join("stale.ready");
    let stale_go = control_dir.path().join("stale.go");
    let stale_result = control_dir.path().join("stale.result");
    let stale_child = spawn_client_child(
        &codex_home,
        &server_url,
        &stale_ready,
        &stale_go,
        &stale_result,
    )?;
    wait_for_paths(std::slice::from_ref(&stale_ready)).await?;

    // The held process has already copied the old credentials into RMCP memory.
    // Make the durable copy expired so a second process rotates it.
    seed_tokens(&codex_home, &server_url, /*expires_at*/ 0).await?;
    let rotator_ready = control_dir.path().join("rotator.ready");
    let rotator_go = control_dir.path().join("rotator.go");
    let rotator_result = control_dir.path().join("rotator.result");
    let rotator_child = spawn_client_child(
        &codex_home,
        &server_url,
        &rotator_ready,
        &rotator_go,
        &rotator_result,
    )?;
    wait_for_paths(std::slice::from_ref(&rotator_ready)).await?;
    fs::write(&rotator_go, "go")?;
    wait_for_children(vec![rotator_child]).await?;
    assert_eq!(fs::read_to_string(&rotator_result)?, "success");
    assert_eq!(
        persisted_token_snapshot(&codex_home)?,
        PersistedTokenSnapshot {
            access_token: NEW_ACCESS_TOKEN.to_string(),
            refresh_token: Some(NEW_REFRESH_TOKEN.to_string()),
        }
    );

    fs::write(&stale_go, "go")?;
    wait_for_children(vec![stale_child]).await?;
    assert_eq!(fs::read_to_string(&stale_result)?, "success");

    let requests = server
        .received_requests()
        .await
        .context("wiremock request recording disabled")?;
    let mut authorization_headers = requests
        .iter()
        .filter(|request| request.method.as_str() == "POST" && request.url.path() == "/mcp")
        .map(|request| {
            request
                .headers
                .get("authorization")
                .expect("authorization header")
                .to_str()
                .expect("ASCII authorization header")
                .to_string()
        })
        .collect::<Vec<_>>();
    authorization_headers.sort();
    assert_eq!(
        authorization_headers,
        vec![
            format!("Bearer {NEW_ACCESS_TOKEN}"),
            format!("Bearer {NEW_ACCESS_TOKEN}"),
            format!("Bearer {OLD_ACCESS_TOKEN}"),
            format!("Bearer {OLD_ACCESS_TOKEN}"),
        ]
    );

    // initialize() calls Codex's persistor. Because RMCP's stale credentials
    // did not change locally, Codex suppresses the write and preserves the
    // newer durable credentials.
    assert_eq!(
        persisted_token_snapshot(&codex_home)?,
        PersistedTokenSnapshot {
            access_token: NEW_ACCESS_TOKEN.to_string(),
            refresh_token: Some(NEW_REFRESH_TOKEN.to_string()),
        }
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by the OAuth refresh race integration tests"]
async fn oauth_refresh_race_seed_child() -> anyhow::Result<()> {
    let server_url = std::env::var(SERVER_URL_ENV)?;
    let access_token = std::env::var(ACCESS_TOKEN_ENV)?;
    let refresh_token = std::env::var(REFRESH_TOKEN_ENV)?;
    let expires_at = std::env::var(EXPIRES_AT_ENV)?.parse()?;
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(access_token),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(refresh_token)));
    response.set_expires_in(Some(&Duration::from_secs(3600)));
    save_oauth_tokens(
        SERVER_NAME,
        &StoredOAuthTokens {
            server_name: SERVER_NAME.to_string(),
            url: server_url,
            client_id: CLIENT_ID.to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at: Some(expires_at),
        },
        OAuthCredentialsStoreMode::File,
    )?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by the OAuth refresh race integration tests"]
async fn oauth_refresh_race_client_child() -> anyhow::Result<()> {
    let server_url = std::env::var(SERVER_URL_ENV)?;
    let ready_path = PathBuf::from(std::env::var(READY_PATH_ENV)?);
    let go_path = PathBuf::from(std::env::var(GO_PATH_ENV)?);
    let result_path = PathBuf::from(std::env::var(RESULT_PATH_ENV)?);
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

    fs::write(ready_path, "ready")?;
    wait_for_paths(std::slice::from_ref(&go_path)).await?;
    let outcome = match initialize_client(&client).await {
        Ok(()) => "success".to_string(),
        Err(error) => format!("error:{error:#}"),
    };
    fs::write(result_path, outcome)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by the OAuth refresh race integration tests"]
async fn oauth_refresh_operation_timeout_child() -> anyhow::Result<()> {
    let server_url = std::env::var(SERVER_URL_ENV)?;
    let ready_path = PathBuf::from(std::env::var(READY_PATH_ENV)?);
    let go_path = PathBuf::from(std::env::var(GO_PATH_ENV)?);
    let result_path = PathBuf::from(std::env::var(RESULT_PATH_ENV)?);
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

    fs::write(ready_path, "ready")?;
    wait_for_paths(std::slice::from_ref(&go_path)).await?;
    let outcome = match client.list_tools(None, Some(OPERATION_TIMEOUT)).await {
        Ok(_) => "success".to_string(),
        Err(error) => format!("error:{error:#}"),
    };
    fs::write(result_path, outcome)?;
    Ok(())
}

async fn mount_oauth_metadata(server: &MockServer, expected_requests: u64) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(expected_requests)
        .mount(server)
        .await;
}

async fn mount_mcp_server(server: &MockServer, expected_requests: u64) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let Ok(body) = request.body_json::<Value>() else {
                return ResponseTemplate::new(400).set_body_string("invalid JSON-RPC request");
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
                            "name": "oauth-refresh-race-test",
                            "version": "0.0.0-test",
                        },
                    },
                })),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(expected_requests)
        .mount(server)
        .await;
}

async fn seed_tokens(
    codex_home: &TempDir,
    server_url: &str,
    expires_at: u64,
) -> anyhow::Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "oauth_refresh_race_seed_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(SERVER_URL_ENV, server_url)
        .env(ACCESS_TOKEN_ENV, OLD_ACCESS_TOKEN)
        .env(REFRESH_TOKEN_ENV, OLD_REFRESH_TOKEN)
        .env(EXPIRES_AT_ENV, expires_at.to_string())
        .kill_on_drop(true);
    let status = timeout(CHILD_WAIT_TIMEOUT, command.status())
        .await
        .context("timed out waiting for OAuth token seed child")??;
    anyhow::ensure!(status.success(), "OAuth token seed child failed: {status}");
    Ok(())
}

fn spawn_client_child(
    codex_home: &TempDir,
    server_url: &str,
    ready_path: &Path,
    go_path: &Path,
    result_path: &Path,
) -> anyhow::Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "oauth_refresh_race_client_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(SERVER_URL_ENV, server_url)
        .env(READY_PATH_ENV, ready_path)
        .env(GO_PATH_ENV, go_path)
        .env(RESULT_PATH_ENV, result_path)
        .kill_on_drop(true);
    Ok(command.spawn()?)
}

fn spawn_operation_timeout_child(
    codex_home: &TempDir,
    server_url: &str,
    ready_path: &Path,
    go_path: &Path,
    result_path: &Path,
) -> anyhow::Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "oauth_refresh_operation_timeout_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(SERVER_URL_ENV, server_url)
        .env(READY_PATH_ENV, ready_path)
        .env(GO_PATH_ENV, go_path)
        .env(RESULT_PATH_ENV, result_path)
        .kill_on_drop(true);
    Ok(command.spawn()?)
}

async fn wait_for_children(children: Vec<Child>) -> anyhow::Result<()> {
    for mut child in children {
        let status = timeout(CHILD_WAIT_TIMEOUT, child.wait())
            .await
            .context("timed out waiting for OAuth client child")??;
        anyhow::ensure!(status.success(), "OAuth client child failed: {status}");
    }
    Ok(())
}

async fn wait_for_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    timeout(Duration::from_secs(10), async {
        loop {
            if paths.iter().all(|path| path.exists()) {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for OAuth race process barrier")
}

fn read_outcomes(paths: &[PathBuf]) -> anyhow::Result<Vec<String>> {
    paths
        .iter()
        .map(fs::read_to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn request_bodies(requests: &[Request], request_path: &str) -> Vec<String> {
    requests
        .iter()
        .filter(|request| request.method.as_str() == "POST" && request.url.path() == request_path)
        .map(|request| String::from_utf8_lossy(&request.body).to_string())
        .collect()
}

fn file_oauth_refresh_lock_path(codex_home: &TempDir, server_url: &str) -> anyhow::Result<PathBuf> {
    let store_key_payload = json!({
        "type": "http",
        "url": server_url,
        "headers": {},
    });
    let account = format!("{SERVER_NAME}|{}", sha_256_prefix(&store_key_payload)?);
    let lock_id = sha_256_prefix(&Value::String(format!("{KEYRING_SERVICE}:{account}")))?;
    Ok(codex_home
        .path()
        .join(format!(".credentials.{lock_id}.refresh.lock")))
}

fn sha_256_prefix(value: &Value) -> anyhow::Result<String> {
    let serialized = serde_json::to_string(value)?;
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let digest = hasher.finalize();
    Ok(format!("{digest:x}")[..16].to_string())
}

#[derive(Debug, Deserialize)]
struct PersistedTokenEntry {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, PartialEq)]
struct PersistedTokenSnapshot {
    access_token: String,
    refresh_token: Option<String>,
}

fn persisted_token_snapshot(codex_home: &TempDir) -> anyhow::Result<PersistedTokenSnapshot> {
    let contents = fs::read_to_string(codex_home.path().join(".credentials.json"))?;
    let entries: BTreeMap<String, PersistedTokenEntry> = serde_json::from_str(&contents)?;
    let entry = entries
        .values()
        .next()
        .context("missing persisted OAuth token entry")?;
    anyhow::ensure!(entries.len() == 1, "expected one persisted OAuth entry");
    Ok(PersistedTokenSnapshot {
        access_token: entry.access_token.clone(),
        refresh_token: entry.refresh_token.clone(),
    })
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
