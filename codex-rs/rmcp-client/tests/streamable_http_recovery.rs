mod streamable_http_test_support;

use std::ffi::OsString;
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
use serial_test::serial;
use tempfile::TempDir;

use streamable_http_test_support::arm_session_post_failure;
use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::create_client;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::initialize_client;
use streamable_http_test_support::initialize_client_with_timeout;
use streamable_http_test_support::spawn_streamable_http_server;
use streamable_http_test_support::spawn_streamable_http_server_with_env;

const OAUTH_TEST_SERVER_NAME: &str = "test-streamable-http-oauth";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const VALID_REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const ROTATED_REFRESH_TOKEN: &str = "rotated-refresh-token";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_404_session_expiry_recovers_and_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 404,
        /*remaining*/ 1,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let recovered = call_echo_tool(&client, "recovered").await?;
    assert_eq!(recovered, expected_echo_result("recovered"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_401_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 401,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let first_error = call_echo_tool(&client, "unauthorized").await.unwrap_err();
    assert!(first_error.to_string().contains("401"));

    let second_error = call_echo_tool(&client, "still-unauthorized")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("401"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_403_scope_challenge_returns_insufficient_scope() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 403,
        /*remaining*/ 1,
        /*www_authenticate_headers*/
        &[r#"Bearer error="insufficient_scope", scope="files:read files:write""#],
    )
    .await?;

    let error = call_echo_tool(&client, "forbidden").await.unwrap_err();
    assert!(
        error.to_string().contains("Insufficient scope"),
        "expected insufficient-scope transport error, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_403_finds_bearer_challenge_in_later_header_value() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 403,
        /*remaining*/ 1,
        /*www_authenticate_headers*/
        &[
            r#"Basic realm="example""#,
            r#"Bearer error="insufficient_scope", scope="files:read""#,
        ],
    )
    .await?;

    let error = call_echo_tool(&client, "forbidden").await.unwrap_err();
    assert!(
        error.to_string().contains("Insufficient scope"),
        "expected insufficient-scope transport error, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_404_recovery_only_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 404,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let error = call_echo_tool(&client, "double-404").await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("handshaking with MCP server failed")
            || error.to_string().contains("Transport channel closed")
    );

    let recovered = call_echo_tool(&client, "after-double-404").await?;
    assert_eq!(recovered, expected_echo_result("after-double-404"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_refreshes_expired_token_before_initialize() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
    ])
    .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url)?;

    let client = RmcpClient::new_streamable_http_client(
        OAUTH_TEST_SERVER_NAME,
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

    let result = call_echo_tool(&client, "after-refresh").await?;
    assert_eq!(result, expected_echo_result("after-refresh"));

    let credentials = std::fs::read_to_string(codex_home.dir.path().join(".credentials.json"))?;
    assert!(credentials.contains(REFRESHED_ACCESS_TOKEN));
    assert!(credentials.contains(ROTATED_REFRESH_TOKEN));
    assert!(!credentials.contains(EXPIRED_ACCESS_TOKEN));
    assert!(!credentials.contains(VALID_REFRESH_TOKEN));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_refresh_respects_initialize_timeout() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "200"),
    ])
    .await?;
    let _codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url)?;

    let client = RmcpClient::new_streamable_http_client(
        OAUTH_TEST_SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    let error = initialize_client_with_timeout(&client, Some(Duration::from_millis(50)))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("timed out handshaking with MCP server after 50ms"),
        "expected initialize timeout, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_non_session_failure_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 500,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let first_error = call_echo_tool(&client, "server-error").await.unwrap_err();
    assert!(first_error.to_string().contains("500"));

    let second_error = call_echo_tool(&client, "still-server-error")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("500"));

    Ok(())
}

struct TempCodexHome {
    original: Option<OsString>,
    dir: TempDir,
}

impl TempCodexHome {
    fn new() -> anyhow::Result<Self> {
        let original = std::env::var_os("CODEX_HOME");
        let dir = TempDir::new()?;
        unsafe {
            std::env::set_var("CODEX_HOME", dir.path());
        }
        Ok(Self { original, dir })
    }
}

impl Drop for TempCodexHome {
    fn drop(&mut self) {
        unsafe {
            if let Some(original) = &self.original {
                std::env::set_var("CODEX_HOME", original);
            } else {
                std::env::remove_var("CODEX_HOME");
            }
        }
    }
}

fn save_expired_oauth_tokens(server_url: &str) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(VALID_REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));

    let tokens = StoredOAuthTokens {
        server_name: OAUTH_TEST_SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        OAUTH_TEST_SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
    )
}
