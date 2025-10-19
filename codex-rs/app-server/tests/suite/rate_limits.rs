use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::RequestId;
use codex_core::auth::AuthDotJson;
use codex_core::auth::get_auth_file;
use codex_core::auth::write_auth_json;
use codex_core::token_data::TokenData;
use codex_core::token_data::parse_id_token;
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

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_requires_auth() {
    let codex_home = TempDir::new().unwrap();

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
async fn get_account_rate_limits_requires_chatgpt_auth() {
    let codex_home = TempDir::new().unwrap();

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_account_rate_limits_returns_snapshot() {
    let codex_home = TempDir::new().unwrap();
    write_chatgpt_auth(codex_home.path(), "chatgpt-token", "account-123")
        .expect("write chatgpt auth");

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url).expect("write chatgpt base url");

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

    let expected = GetAccountRateLimitsResponse {
        rate_limits: RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 42.0,
                window_minutes: Some(60),
                resets_at: Some(primary_reset_iso.to_string()),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 5.0,
                window_minutes: Some(1440),
                resets_at: Some(secondary_reset_iso.to_string()),
            }),
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

fn write_chatgpt_base_url(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, format!("chatgpt_base_url = \"{base_url}\"\n"))
}

fn write_chatgpt_auth(
    codex_home: &Path,
    access_token: &str,
    account_id: &str,
) -> std::io::Result<()> {
    let auth_path = get_auth_file(codex_home);
    let id_token_raw = encode_chatgpt_id_token("pro");
    let id_token = parse_id_token(&id_token_raw).map_err(std::io::Error::other)?;
    let auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token,
            access_token: access_token.to_string(),
            refresh_token: "refresh-token".to_string(),
            account_id: Some(account_id.to_string()),
        }),
        last_refresh: Some(Utc::now()),
    };
    write_auth_json(&auth_path, &auth)
}

fn encode_chatgpt_id_token(plan_type: &str) -> String {
    let header = json!({ "alg": "none", "typ": "JWT" });
    let payload = json!({
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": plan_type
        }
    });
    let header_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&header).unwrap_or_else(|err| panic!("serialize jwt header: {err}")),
    );
    let payload_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&payload).unwrap_or_else(|err| panic!("serialize jwt payload: {err}")),
    );
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"signature");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}
