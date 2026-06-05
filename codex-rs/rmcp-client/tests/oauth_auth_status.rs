mod streamable_http_test_support;

use std::fs;
use std::net::TcpListener;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_protocol::protocol::McpAuthStatus;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::determine_streamable_http_auth_status;
use codex_rmcp_client::save_oauth_tokens;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-oauth-auth-status";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_AUTH_STATUS_SCENARIO";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_AUTH_STATUS_SERVER_URL";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn blank_access_tokens_are_not_logged_in_without_discovery() -> anyhow::Result<()> {
    run_child("empty_access_token", "not-a-valid-url").await?;
    run_child("whitespace_access_token", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn blank_client_ids_are_not_logged_in_without_discovery() -> anyhow::Result<()> {
    run_child("empty_client_id", "not-a-valid-url").await?;
    run_child("whitespace_client_id", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn expired_credentials_require_nonblank_refresh_token() -> anyhow::Result<()> {
    run_child("expired_without_refresh", "not-a-valid-url").await?;
    run_child("expired_with_empty_refresh", "not-a-valid-url").await?;
    run_child("expired_with_whitespace_refresh", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshable_expired_credentials_report_oauth() -> anyhow::Result<()> {
    run_child("expired_with_refresh", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_window_boundary_matches_startup_refresh() -> anyhow::Result<()> {
    run_child("inside_refresh_window", "not-a-valid-url").await?;
    run_child("outside_refresh_window", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn future_credentials_without_refresh_report_oauth_without_discovery() -> anyhow::Result<()> {
    run_child("future_without_refresh", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn legacy_credentials_without_expiry_report_oauth_without_discovery() -> anyhow::Result<()> {
    run_child("legacy_without_expiry", "not-a-valid-url").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn server_rejected_credentials_still_report_oauth() -> anyhow::Result<()> {
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
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            "Bearer server-rejected-access-token",
        ))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let server_url = format!("{}/mcp", server.uri());
    run_child("server_rejected", &server_url).await?;

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unreachable_server_with_credentials_still_reports_oauth() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let unreachable_url = format!("http://{}/mcp", listener.local_addr()?);
    drop(listener);
    run_child("unreachable_server", &unreachable_url).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn malformed_oauth_store_returns_an_error_instead_of_oauth() -> anyhow::Result<()> {
    run_child("malformed_store", "not-a-valid-url").await
}

async fn run_child(scenario: &str, server_url: &str) -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_auth_status_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SCENARIO_ENV, scenario)
        .env(CHILD_SERVER_URL_ENV, server_url)
        .status()
        .await?;

    assert!(status.success(), "auth status child failed: {status}");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by OAuth auth status tests"]
async fn oauth_auth_status_child() -> anyhow::Result<()> {
    let scenario = std::env::var(CHILD_SCENARIO_ENV)?;
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;

    if scenario == "malformed_store" {
        let codex_home = std::env::var("CODEX_HOME")?;
        fs::write(
            std::path::Path::new(&codex_home).join(".credentials.json"),
            "{not valid json",
        )?;

        let error = determine_auth_status(&server_url)
            .await
            .expect_err("malformed credentials should not report OAuth");
        let message = format!("{error:#}");
        assert!(
            message.contains("failed to parse credentials file"),
            "unexpected error: {message}"
        );
        return Ok(());
    }

    if scenario == "legacy_without_expiry" {
        let response = OAuthTokenResponse::new(
            AccessToken::new("access-token".to_string()),
            BasicTokenType::Bearer,
            VendorExtraTokenFields::default(),
        );
        save_oauth_tokens(
            SERVER_NAME,
            &StoredOAuthTokens {
                server_name: SERVER_NAME.to_string(),
                url: server_url.clone(),
                client_id: "client-id".to_string(),
                token_response: WrappedOAuthTokenResponse(response),
                expires_at: None,
            },
            OAuthCredentialsStoreMode::File,
        )?;

        assert_eq!(
            determine_auth_status(&server_url).await?,
            McpAuthStatus::OAuth
        );
        return Ok(());
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;
    let (access_token, client_id, refresh_token, expires_at, expected_status) =
        match scenario.as_str() {
            "empty_access_token" => (
                "",
                "client-id",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::NotLoggedIn,
            ),
            "whitespace_access_token" => (
                " \t ",
                "client-id",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::NotLoggedIn,
            ),
            "empty_client_id" => (
                "access-token",
                "",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::NotLoggedIn,
            ),
            "whitespace_client_id" => (
                "access-token",
                " \t ",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::NotLoggedIn,
            ),
            "expired_without_refresh" => (
                "expired-access-token",
                "client-id",
                /*refresh_token*/ None,
                Some(0),
                McpAuthStatus::NotLoggedIn,
            ),
            "expired_with_empty_refresh" => (
                "expired-access-token",
                "client-id",
                Some(""),
                Some(0),
                McpAuthStatus::NotLoggedIn,
            ),
            "expired_with_whitespace_refresh" => (
                "expired-access-token",
                "client-id",
                Some(" \t "),
                Some(0),
                McpAuthStatus::NotLoggedIn,
            ),
            "server_rejected" => (
                "server-rejected-access-token",
                "client-id",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::OAuth,
            ),
            "unreachable_server" => (
                "access-token",
                "client-id",
                /*refresh_token*/ None,
                /*expires_at*/ None,
                McpAuthStatus::OAuth,
            ),
            "expired_with_refresh" => (
                "expired-access-token",
                "client-id",
                Some("refresh-token"),
                Some(0),
                McpAuthStatus::OAuth,
            ),
            "inside_refresh_window" => (
                "",
                "client-id",
                Some("refresh-token"),
                Some(now_ms.saturating_add(29_000)),
                McpAuthStatus::OAuth,
            ),
            "outside_refresh_window" => (
                "",
                "client-id",
                Some("refresh-token"),
                Some(now_ms.saturating_add(31_000)),
                McpAuthStatus::NotLoggedIn,
            ),
            "future_without_refresh" => (
                "access-token",
                "client-id",
                /*refresh_token*/ None,
                Some(u64::MAX),
                McpAuthStatus::OAuth,
            ),
            unexpected => panic!("unexpected child scenario: {unexpected}"),
        };
    save_tokens(
        &server_url,
        access_token,
        client_id,
        refresh_token,
        expires_at,
    )?;

    assert_eq!(determine_auth_status(&server_url).await?, expected_status);

    match scenario.as_str() {
        "server_rejected" => {
            let client = new_client(&server_url).await?;
            assert!(
                initialize_client(&client).await.is_err(),
                "{scenario} credentials unexpectedly initialized"
            );
        }
        "unreachable_server" => {
            if let Ok(client) = new_client(&server_url).await {
                assert!(
                    initialize_client(&client).await.is_err(),
                    "unreachable server unexpectedly initialized"
                );
            }
        }
        "empty_access_token"
        | "whitespace_access_token"
        | "empty_client_id"
        | "whitespace_client_id"
        | "expired_without_refresh"
        | "expired_with_empty_refresh"
        | "expired_with_whitespace_refresh"
        | "expired_with_refresh"
        | "inside_refresh_window"
        | "outside_refresh_window"
        | "future_without_refresh" => {}
        unexpected => panic!("unexpected child scenario: {unexpected}"),
    }

    Ok(())
}

fn save_tokens(
    server_url: &str,
    access_token: &str,
    client_id: &str,
    refresh_token: Option<&str>,
    expires_at: Option<u64>,
) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(access_token.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    if let Some(refresh_token) = refresh_token {
        response.set_refresh_token(Some(RefreshToken::new(refresh_token.to_string())));
    }
    response.set_expires_in(Some(&Duration::from_secs(7200)));

    save_oauth_tokens(
        SERVER_NAME,
        &StoredOAuthTokens {
            server_name: SERVER_NAME.to_string(),
            url: server_url.to_string(),
            client_id: client_id.to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at,
        },
        OAuthCredentialsStoreMode::File,
    )
}

async fn determine_auth_status(server_url: &str) -> anyhow::Result<McpAuthStatus> {
    determine_streamable_http_auth_status(
        SERVER_NAME,
        server_url,
        /*bearer_token_env_var*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
    )
    .await
}

async fn new_client(server_url: &str) -> anyhow::Result<RmcpClient> {
    RmcpClient::new_streamable_http_client(
        SERVER_NAME,
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
