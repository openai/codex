mod streamable_http_test_support;

use std::ffi::OsString;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::delete_oauth_tokens_async;
use codex_rmcp_client::save_oauth_tokens_async;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serial_test::serial;
use tempfile::TempDir;
use tokio::time::sleep;

use streamable_http_test_support::arm_session_post_failure;
use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::create_client;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::initialize_client;
use streamable_http_test_support::initialize_client_with_timeout;
use streamable_http_test_support::spawn_oauth_client_process;
use streamable_http_test_support::spawn_oauth_credential_writer_process;
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
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;
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
async fn streamable_http_oauth_preserves_refresh_token_when_refresh_response_omits_rotation()
-> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_OMIT_ROTATED_REFRESH_TOKEN", "1"),
    ])
    .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;
    initialize_client(&client).await?;

    let credentials = std::fs::read_to_string(codex_home.dir.path().join(".credentials.json"))?;
    assert!(credentials.contains(REFRESHED_ACCESS_TOKEN));
    assert!(credentials.contains(VALID_REFRESH_TOKEN));
    assert!(!credentials.contains(EXPIRED_ACCESS_TOKEN));
    assert!(!credentials.contains(ROTATED_REFRESH_TOKEN));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_concurrent_initializes_share_refreshed_credentials()
-> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_MAX_USES", "1"),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "50"),
    ])
    .await?;
    let _codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client_a = create_oauth_file_client(&server_url).await?;
    let client_b = create_oauth_file_client(&server_url).await?;

    let (initialized_a, initialized_b) =
        tokio::join!(initialize_client(&client_a), initialize_client(&client_b));
    initialized_a?;
    initialized_b?;

    let result_a = call_echo_tool(&client_a, "after-shared-refresh-a").await?;
    let result_b = call_echo_tool(&client_b, "after-shared-refresh-b").await?;
    assert_eq!(result_a, expected_echo_result("after-shared-refresh-a"));
    assert_eq!(result_b, expected_echo_result("after-shared-refresh-b"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_cross_process_waits_for_slow_refresh() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_MAX_USES", "1"),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "5200"),
    ])
    .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let mut first =
        spawn_oauth_client_process(OAUTH_TEST_SERVER_NAME, &server_url, codex_home.dir.path())?;
    sleep(Duration::from_millis(100)).await;
    let mut second =
        spawn_oauth_client_process(OAUTH_TEST_SERVER_NAME, &server_url, codex_home.dir.path())?;

    let (first_status, second_status) = tokio::join!(first.wait(), second.wait());
    assert!(first_status?.success());
    assert!(second_status?.success());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_file_writes_are_serialized_across_servers() -> anyhow::Result<()> {
    const WRITER_COUNT: usize = 12;

    let codex_home = TempCodexHome::new()?;
    let barrier = codex_home.dir.path().join("credential-write-barrier");
    let mut writers = Vec::new();
    for index in 0..WRITER_COUNT {
        writers.push(spawn_oauth_credential_writer_process(
            &format!("server-{index}"),
            &format!("https://example.com/mcp/{index}"),
            &format!("access-{index}"),
            &format!("refresh-{index}"),
            codex_home.dir.path(),
            &barrier,
        )?);
    }

    sleep(Duration::from_millis(300)).await;
    std::fs::write(&barrier, "")?;
    for writer in &mut writers {
        assert!(writer.wait().await?.success());
    }

    let credentials = std::fs::read_to_string(codex_home.dir.path().join(".credentials.json"))?;
    let entries = serde_json::from_str::<serde_json::Value>(&credentials)?;
    let entries = entries
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("credentials file should contain an object"))?;
    assert_eq!(entries.len(), WRITER_COUNT);
    for index in 0..WRITER_COUNT {
        assert!(
            entries
                .values()
                .any(|entry| entry["server_name"] == format!("server-{index}"))
        );
    }

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_unexpired_token_does_not_require_writable_codex_home()
-> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let (_server, base_url) =
        spawn_streamable_http_server_with_env(&[("MCP_EXPECT_BEARER", "unexpired-access-token")])
            .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .checked_add(Duration::from_secs(7200))
        .ok_or_else(|| anyhow::anyhow!("expiry overflow"))?
        .as_millis() as u64;
    save_test_oauth_tokens(
        OAUTH_TEST_SERVER_NAME,
        &server_url,
        "unexpired-access-token",
        VALID_REFRESH_TOKEN,
        expires_at,
    )
    .await?;
    std::fs::remove_dir_all(codex_home.dir.path().join(".mcp-oauth-locks"))?;
    std::fs::set_permissions(
        codex_home.dir.path(),
        std::fs::Permissions::from_mode(0o500),
    )?;

    let result = async {
        let client = create_oauth_file_client(&server_url).await?;
        initialize_client(&client).await
    }
    .await;
    std::fs::set_permissions(
        codex_home.dir.path(),
        std::fs::Permissions::from_mode(0o700),
    )?;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_refresh_timeout_keeps_refresh_running() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "300"),
    ])
    .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;

    let error = initialize_client_with_timeout(&client, Duration::from_millis(50))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("timed out handshaking with MCP server after 50ms"),
        "expected initialize timeout, got: {error:#}"
    );

    let credentials_path = codex_home.dir.path().join(".credentials.json");
    let credentials_backup_path = codex_home.dir.path().join(".credentials.json.backup");
    std::fs::rename(&credentials_path, &credentials_backup_path)?;
    std::fs::create_dir(&credentials_path)?;
    sleep(Duration::from_millis(375)).await;
    std::fs::remove_dir(&credentials_path)?;
    std::fs::rename(&credentials_backup_path, &credentials_path)?;

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let credentials = std::fs::read_to_string(&credentials_path)?;
        if credentials.contains(REFRESHED_ACCESS_TOKEN)
            && credentials.contains(ROTATED_REFRESH_TOKEN)
            && !credentials.contains(EXPIRED_ACCESS_TOKEN)
        {
            break;
        }

        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for background refresh to persist credentials");
        }
        sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_logout_wins_against_detached_refresh() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "200"),
    ])
    .await?;
    let codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;
    initialize_client_with_timeout(&client, Duration::from_millis(50))
        .await
        .unwrap_err();

    assert!(
        delete_oauth_tokens_async(
            OAUTH_TEST_SERVER_NAME,
            &server_url,
            OAuthCredentialsStoreMode::File,
        )
        .await?
    );
    sleep(Duration::from_millis(100)).await;
    assert!(!codex_home.dir.path().join(".credentials.json").exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_refresh_and_initialize_share_timeout_budget() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_TOKEN_DELAY_MS", "100"),
        ("MCP_INITIALIZE_DELAY_MS", "200"),
    ])
    .await?;
    let _codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;

    let error = initialize_client_with_timeout(&client, Duration::from_millis(250))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("timed out handshaking with MCP server after 250ms"),
        "expected initialize timeout, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_transient_refresh_failure_does_not_require_login()
-> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", VALID_REFRESH_TOKEN),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
        ("MCP_REFRESH_ERROR", "server_error"),
    ])
    .await?;
    let _codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;

    let error = initialize_client(&client).await.unwrap_err();
    assert!(!error.to_string().contains("Auth required"));
    assert!(error.to_string().contains("failed to refresh OAuth tokens"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(oauth_credentials_env)]
async fn streamable_http_oauth_refresh_failure_reports_auth_required() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server_with_env(&[
        ("MCP_EXPECT_BEARER", REFRESHED_ACCESS_TOKEN),
        ("MCP_EXPECT_REFRESH_TOKEN", "different-refresh-token"),
        ("MCP_REFRESH_ACCESS_TOKEN", REFRESHED_ACCESS_TOKEN),
        ("MCP_ROTATED_REFRESH_TOKEN", ROTATED_REFRESH_TOKEN),
    ])
    .await?;
    let _codex_home = TempCodexHome::new()?;
    let server_url = format!("{base_url}/mcp");
    save_expired_oauth_tokens(&server_url).await?;

    let client = create_oauth_file_client(&server_url).await?;

    let error = initialize_client(&client).await.unwrap_err();
    assert!(
        error.to_string().contains("Auth required"),
        "expected auth-required error, got: {error:#}"
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

async fn save_expired_oauth_tokens(server_url: &str) -> anyhow::Result<()> {
    save_test_oauth_tokens(
        OAUTH_TEST_SERVER_NAME,
        server_url,
        EXPIRED_ACCESS_TOKEN,
        VALID_REFRESH_TOKEN,
        /*expires_at*/ 0,
    )
    .await
}

async fn create_oauth_file_client(server_url: &str) -> anyhow::Result<RmcpClient> {
    RmcpClient::new_streamable_http_client(
        OAUTH_TEST_SERVER_NAME,
        server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await
}

async fn save_test_oauth_tokens(
    server_name: &str,
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

    let tokens = StoredOAuthTokens {
        server_name: server_name.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(expires_at),
    };
    save_oauth_tokens_async(server_name, &tokens, OAuthCredentialsStoreMode::File).await
}
