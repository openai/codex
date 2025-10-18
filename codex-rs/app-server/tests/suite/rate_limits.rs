use app_test_support::ConfigBuilder;
use app_test_support::DEFAULT_READ_TIMEOUT;
use app_test_support::McpProcess;
use app_test_support::login_with_api_key_via_mcp;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|err| panic!("create tempdir: {err}"));
    ConfigBuilder::default()
        .with_defaults()
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .expect("send account/rateLimits/read");

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("account/rateLimits/read timeout")
    .expect("account/rateLimits/read error");

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "codex account authentication required to read rate limits"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_returns_snapshot() {
    let codex_home = TempDir::new().unwrap_or_else(|err| panic!("create tempdir: {err}"));
    let server = MockServer::start().await;

    let primary_reset_iso = "2025-01-01T00:02:00Z";
    let secondary_reset_iso = "2025-01-01T01:00:00Z";
    let response_body = json!({
        "plan_type": "pro",
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": {
                "used_percent": 42,
                "limit_window_seconds": 3600,
                "reset_after_seconds": 120,
                "reset_at": primary_reset_iso,
            },
            "secondary_window": {
                "used_percent": 5,
                "limit_window_seconds": 86400,
                "reset_after_seconds": 43200,
                "reset_at": secondary_reset_iso,
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

    let base_url = server.uri();
    ConfigBuilder::default()
        .with_defaults()
        .chatgpt_base_url(base_url.as_str())
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));
    write_chatgpt_auth(codex_home.path(), "chatgpt-token", "account-123", "pro")
        .unwrap_or_else(|err| panic!("write chatgpt auth: {err}"));

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .expect("send account/rateLimits/read");

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("account/rateLimits/read timeout")
    .expect("account/rateLimits/read response");

    let received: GetAccountRateLimitsResponse =
        to_response(response).expect("deserialize rate limit response");
    let primary_reset_epoch = DateTime::parse_from_rfc3339(primary_reset_iso)
        .unwrap_or_else(|err| panic!("parse primary reset: {err}"))
        .with_timezone(&Utc)
        .timestamp();
    let secondary_reset_epoch = DateTime::parse_from_rfc3339(secondary_reset_iso)
        .unwrap_or_else(|err| panic!("parse secondary reset: {err}"))
        .with_timezone(&Utc)
        .timestamp();

    let expected = GetAccountRateLimitsResponse {
        rate_limits: RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 42.0,
                window_minutes: Some(60),
                resets_in_seconds: Some(120),
                resets_at: Some(primary_reset_epoch),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 5.0,
                window_minutes: Some(1440),
                resets_in_seconds: Some(43_200),
                resets_at: Some(secondary_reset_epoch),
            }),
        },
    };
    assert_eq!(received, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_chatgpt_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|err| panic!("create tempdir: {err}"));
    ConfigBuilder::default()
        .with_defaults()
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    login_with_api_key_via_mcp(&mut mcp, "sk-test-key")
        .await
        .expect("login with api key");

    let request_id = mcp
        .send_get_account_rate_limits_request()
        .await
        .expect("send account/rateLimits/read");

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("account/rateLimits/read timeout")
    .expect("account/rateLimits/read error");

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "chatgpt authentication required to read rate limits"
    );
}
