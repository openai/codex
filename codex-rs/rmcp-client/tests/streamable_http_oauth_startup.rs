mod streamable_http_test_support;

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::McpAuthStatus;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::determine_streamable_http_auth_status;
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
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tracing::Event;
use tracing::Id;
use tracing::Metadata;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Record;
use tracing::subscriber::Interest;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;
use streamable_http_test_support::initialize_client_with_timeout;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const ROTATED_REFRESH_TOKEN: &str = "rotated-refresh-token";
const CHILD_CONTENTION_FILE_ENV: &str = "MCP_TEST_OAUTH_STARTUP_CONTENTION_FILE";
const CHILD_READY_FILE_ENV: &str = "MCP_TEST_OAUTH_STARTUP_READY_FILE";
const CHILD_RELEASE_FILE_ENV: &str = "MCP_TEST_OAUTH_STARTUP_RELEASE_FILE";
const PREFLIGHT_REFRESH_ERROR: &str = "preflight refresh failed distinctly";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
// This mirrors the private event target in oauth::refresh_lock without exposing test-only crate API.
const LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::refresh_lock::contention";
const UNREFRESHABLE_SERVER_URL: &str = "https://unrefreshable.example/mcp";
const UNEXPIRED_SERVER_URL: &str = "https://unexpired.example/mcp";
const REFRESHABLE_SERVER_URL: &str = "https://refreshable.example/mcp";

#[derive(Clone)]
struct GatedRefreshState {
    request_count: Arc<AtomicUsize>,
    request_started: Arc<Notify>,
    response_release: Arc<Semaphore>,
}

struct GatedRefreshServer {
    token_endpoint: String,
    state: GatedRefreshState,
    task: JoinHandle<()>,
}

impl GatedRefreshServer {
    async fn start() -> anyhow::Result<Self> {
        let state = GatedRefreshState {
            request_count: Arc::new(AtomicUsize::new(/*v*/ 0)),
            request_started: Arc::new(Notify::new()),
            response_release: Arc::new(Semaphore::new(/*permits*/ 0)),
        };
        let router = Router::new()
            .route("/oauth/token", post(gated_refresh_response))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                panic!("gated refresh server failed: {error}");
            }
        });
        Ok(Self {
            token_endpoint: format!("http://{address}/oauth/token"),
            state,
            task,
        })
    }

    async fn wait_until_request_started(&self) -> anyhow::Result<()> {
        let notified = self.state.request_started.notified();
        if self.request_count() == 0 {
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), notified)
                .await
                .map_err(|_| anyhow::anyhow!("provider refresh request did not start"))?;
        }
        Ok(())
    }

    fn release_responses(&self) {
        // Two permits also let the no-lock negative control exit cleanly after issuing two refresh
        // requests. The passing path consumes one permit because only the lock owner calls out.
        self.state.response_release.add_permits(/*n*/ 2);
    }

    fn request_count(&self) -> usize {
        self.state.request_count.load(Ordering::SeqCst)
    }
}

impl Drop for GatedRefreshServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn gated_refresh_response(
    State(state): State<GatedRefreshState>,
    body: String,
) -> Json<Value> {
    assert!(body.contains("grant_type=refresh_token"));
    assert!(body.contains(&format!("refresh_token={REFRESH_TOKEN}")));
    state.request_count.fetch_add(/*val*/ 1, Ordering::SeqCst);
    state.request_started.notify_one();
    let Ok(permit) = state.response_release.acquire().await else {
        panic!("gated refresh server closed its response semaphore");
    };
    permit.forget();
    Json(json!({
        "access_token": REFRESHED_ACCESS_TOKEN,
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": ROTATED_REFRESH_TOKEN,
    }))
}

struct LockContentionMarkerSubscriber {
    marker_file: PathBuf,
}

impl Subscriber for LockContentionMarkerSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == LOCK_CONTENTION_EVENT_TARGET
    }

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.enabled(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::DEBUG)
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        // This subscriber enables only the contention event callsite, so it never observes spans.
        Id::from_u64(/*u*/ 1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if event.metadata().target() == LOCK_CONTENTION_EVENT_TARGET
            && let Err(error) = fs::write(&self.marker_file, b"contended")
        {
            panic!("failed to write refresh-lock contention marker: {error}");
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

async fn wait_for_marker(path: &Path, timeout_message: &str) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("{timeout_message}"))
}

fn oauth_concurrency_child_command(
    codex_home: &Path,
    server_url: &str,
    ready_file: &Path,
    release_file: &Path,
) -> anyhow::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["oauth_concurrency_client_child", "--exact", "--ignored"])
        .env("CODEX_HOME", codex_home)
        .env(CHILD_SERVER_URL_ENV, server_url)
        .env(CHILD_READY_FILE_ENV, ready_file)
        .env(CHILD_RELEASE_FILE_ENV, release_file)
        .kill_on_drop(true);
    Ok(command)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn startup_refresh_does_not_consume_handshake_timeout() -> anyhow::Result<()> {
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
        // The provider takes longer than the configured MCP handshake timeout. Refresh has its own
        // bound, so this delay must not leave the subsequent handshake with an expired budget.
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(1_500))
                .set_body_json(json!({
                    "access_token": REFRESHED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": REFRESH_TOKEN,
                })),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_file_mode_startup_refreshes_once() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let refresh_server = GatedRefreshServer::start().await?;
    let codex_home = TempDir::new()?;
    let server_url = format!("{}/mcp", server.uri());
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": refresh_server.token_endpoint.clone(),
            "scopes_supported": [""],
        })))
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
        .expect(4)
        .mount(&server)
        .await;

    let seed_status = Command::new(std::env::current_exe()?)
        .args(["oauth_concurrency_seed_child", "--exact", "--ignored"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, &server_url)
        .status()
        .await?;
    assert!(
        seed_status.success(),
        "OAuth concurrency seed child failed: {seed_status}"
    );

    let first_ready_file = codex_home.path().join("oauth-client-first.ready");
    let first_release_file = codex_home.path().join("oauth-client-first.release");
    let second_ready_file = codex_home.path().join("oauth-client-second.ready");
    let second_release_file = codex_home.path().join("oauth-client-second.release");
    let contention_file = codex_home.path().join("oauth-client-second.contended");
    let mut first_child = oauth_concurrency_child_command(
        codex_home.path(),
        &server_url,
        &first_ready_file,
        &first_release_file,
    )?
    .spawn()?;
    let mut second_command = oauth_concurrency_child_command(
        codex_home.path(),
        &server_url,
        &second_ready_file,
        &second_release_file,
    )?;
    second_command.env(CHILD_CONTENTION_FILE_ENV, &contention_file);
    let mut second_child = second_command.spawn()?;

    wait_for_marker(
        &first_ready_file,
        "first OAuth concurrency child did not become ready",
    )
    .await?;
    wait_for_marker(
        &second_ready_file,
        "second OAuth concurrency child did not become ready",
    )
    .await?;

    fs::write(&first_release_file, b"release")?;
    refresh_server.wait_until_request_started().await?;

    // The first child is now inside the provider request while retaining the credential lock.
    // Releasing the second child must make its first try_lock call observe WouldBlock. Keep the
    // provider response gated until that exact branch emits the contention marker.
    fs::write(&second_release_file, b"release")?;
    let contention_result = wait_for_marker(
        &contention_file,
        "second OAuth concurrency child did not observe refresh-lock contention",
    )
    .await;
    refresh_server.release_responses();

    let (first_status, second_status) = tokio::try_join!(first_child.wait(), second_child.wait())?;
    assert!(
        first_status.success(),
        "first OAuth concurrency child failed: {first_status}"
    );
    assert!(
        second_status.success(),
        "second OAuth concurrency child failed: {second_status}"
    );

    server.verify().await;
    contention_result?;
    assert_eq!(refresh_server.request_count(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn operation_preflight_refresh_failure_blocks_rmcp_request() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
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
            "expires_in": 31,
            "refresh_token": ROTATED_REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={ROTATED_REFRESH_TOKEN}"
        )))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": PREFLIGHT_REFRESH_ERROR,
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
                            "name": "oauth-preflight-test",
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
    let status = Command::new(std::env::current_exe()?)
        .args([
            "operation_preflight_refresh_failure_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, &server_url)
        .status()
        .await?;
    assert!(
        status.success(),
        "OAuth preflight failure child failed: {status}"
    );

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
#[ignore = "spawned by reports_auth_status_for_persisted_credentials"]
async fn persisted_credentials_auth_status_child() -> anyhow::Result<()> {
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
    assert_eq!(status, McpAuthStatus::NotLoggedIn);

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
        expires_at: Some(now.saturating_add(/*rhs*/ 60_000)),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(UNEXPIRED_SERVER_URL).await?;
    assert_eq!(status, McpAuthStatus::OAuth);

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
    assert_eq!(status, McpAuthStatus::OAuth);
    Ok(())
}

async fn auth_status(server_url: &str) -> anyhow::Result<McpAuthStatus> {
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
#[ignore = "spawned by startup_refresh_does_not_consume_handshake_timeout"]
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

    initialize_client_with_timeout(&client, Duration::from_secs(1)).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by operation_preflight_refresh_failure_blocks_rmcp_request"]
async fn operation_preflight_refresh_failure_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    save_expired_file_mode_tokens(&server_url)?;

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

    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let error = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await
        .expect_err("preflight refresh failure should abort the operation");
    let message = format!("{error:#}");
    assert!(
        message.contains("failed to refresh OAuth tokens for server"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains(PREFLIGHT_REFRESH_ERROR),
        "unexpected error: {message}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by concurrent_file_mode_startup_refreshes_once"]
async fn oauth_concurrency_seed_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    save_expired_file_mode_tokens(&server_url)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by concurrent_file_mode_startup_refreshes_once"]
async fn oauth_concurrency_client_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let ready_file = PathBuf::from(std::env::var(CHILD_READY_FILE_ENV)?);
    let release_file = PathBuf::from(std::env::var(CHILD_RELEASE_FILE_ENV)?);
    if let Ok(marker_file) = std::env::var(CHILD_CONTENTION_FILE_ENV) {
        tracing::subscriber::set_global_default(LockContentionMarkerSubscriber {
            marker_file: PathBuf::from(marker_file),
        })
        .map_err(|error| anyhow::anyhow!("failed to install contention subscriber: {error}"))?;
    }
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

    // Both processes must construct their OAuth client from the same expired snapshot before the
    // parent releases either one. The parent then gates them separately to force lock contention.
    fs::write(ready_file, b"ready")?;
    while !release_file.exists() {
        tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
    }
    initialize_client(&client).await?;
    Ok(())
}

fn save_expired_file_mode_tokens(server_url: &str) -> anyhow::Result<()> {
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
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    Ok(())
}
