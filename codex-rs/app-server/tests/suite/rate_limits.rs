use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::RequestId;
use codex_protocol::protocol::RateLimitSnapshot;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|err| panic!("create tempdir: {err}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

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
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    login_with_api_key(&mut mcp, "sk-test-key").await;

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
    let expected = GetAccountRateLimitsResponse {
        rate_limits: RateLimitSnapshot {
            primary: None,
            secondary: None,
        },
    };
    assert_eq!(received, expected);
}

#[expect(clippy::expect_used)]
async fn login_with_api_key(mcp: &mut McpProcess, api_key: &str) {
    let request_id = mcp
        .send_login_api_key_request(LoginApiKeyParams {
            api_key: api_key.to_string(),
        })
        .await
        .expect("send loginApiKey");

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("loginApiKey timeout")
    .expect("loginApiKey response");
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
"#,
    )
}
