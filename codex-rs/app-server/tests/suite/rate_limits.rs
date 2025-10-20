use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::RequestId;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_auth() -> Result<()> {
    let codex_home = TempDir::new().context("create codex home tempdir")?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .context("spawn mcp process")?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .context("initialize timeout")?
        .context("initialize request")?;

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .context("send account/rateLimits/read")?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await
    .context("account/rateLimits/read timeout")?
    .context("account/rateLimits/read error")?;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "codex account authentication required to read rate limits"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new().context("create codex home tempdir")?;

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .context("spawn mcp process")?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .context("initialize timeout")?
        .context("initialize request")?;

    login_with_api_key(&mut mcp, "sk-test-key").await?;

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .context("send account/rateLimits/read")?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await
    .context("account/rateLimits/read timeout")?
    .context("account/rateLimits/read error")?;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "chatgpt authentication required to read rate limits"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_returns_snapshot() -> Result<()> {
    let codex_home = TempDir::new().context("create codex home tempdir")?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
    )
    .context("write chatgpt auth")?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url).context("write chatgpt base url")?;

    let primary_reset_timestamp = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:02:00Z")
        .expect("parse primary reset timestamp")
        .timestamp();
    let secondary_reset_timestamp = chrono::DateTime::parse_from_rfc3339("2025-01-01T01:00:00Z")
        .expect("parse secondary reset timestamp")
        .timestamp();
    let response_body = json!({
        "plan_type": "pro",
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": {
                "used_percent": 42,
                "limit_window_seconds": 3600,
                "reset_after_seconds": 120,
                "reset_at": primary_reset_timestamp,
            },
            "secondary_window": {
                "used_percent": 5,
                "limit_window_seconds": 86400,
                "reset_after_seconds": 43200,
                "reset_at": secondary_reset_timestamp,
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .context("spawn mcp process")?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .context("initialize timeout")?
        .context("initialize request")?;

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .context("send account/rateLimits/read")?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .context("account/rateLimits/read timeout")?
    .context("account/rateLimits/read response")?;

    let received: GetAccountRateLimitsResponse =
        to_response(response).context("deserialize rate limit response")?;

    let expected = GetAccountRateLimitsResponse {
        rate_limits: RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 42.0,
                window_minutes: Some(60),
                resets_at: Some(primary_reset_timestamp),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 5.0,
                window_minutes: Some(1440),
                resets_at: Some(secondary_reset_timestamp),
            }),
        },
    };
    assert_eq!(received, expected);

    Ok(())
}

async fn login_with_api_key(mcp: &mut McpProcess, api_key: &str) -> Result<()> {
    let request_id = mcp
        .send_login_api_key_request(LoginApiKeyParams {
            api_key: api_key.to_string(),
        })
        .await
        .context("send loginApiKey")?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .context("loginApiKey timeout")?
    .context("loginApiKey response")?;

    Ok(())
}

fn write_chatgpt_base_url(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, format!("chatgpt_base_url = \"{base_url}\"\n"))
}
