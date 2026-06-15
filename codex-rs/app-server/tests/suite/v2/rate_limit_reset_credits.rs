use std::path::Path;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditParams;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(/*secs*/ 10);
const RATE_LIMIT_RESET_REQUEST_TIMEOUT_ENV_VAR: &str =
    "CODEX_TEST_RATE_LIMIT_RESET_REQUEST_TIMEOUT_MS";
const SERVER_TIMEOUT_READ_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(/*secs*/ 15);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const INTERNAL_ERROR_CODE: i64 = -32603;

#[tokio::test]
async fn consume_rate_limit_reset_credit_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = initialized_app_server(codex_home.path()).await?;

    let consume_id = mcp
        .send_consume_account_rate_limit_reset_credit_request(
            ConsumeAccountRateLimitResetCreditParams {
                idempotency_key: "request-1".to_string(),
            },
        )
        .await?;
    let consume_error = read_error_response(&mut mcp, consume_id).await?;
    assert_eq!(consume_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        consume_error.error.message,
        "codex account authentication required for rate limit reset credits"
    );

    login_with_api_key(&mut mcp, "sk-test-key").await?;
    let consume_id = send_consume_reset_credit(&mut mcp, "request-2").await?;
    let consume_error = read_error_response(&mut mcp, consume_id).await?;
    assert_eq!(consume_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        consume_error.error.message,
        "chatgpt authentication required for rate limit reset credits"
    );
    Ok(())
}

#[tokio::test]
async fn consume_account_rate_limit_reset_credit_maps_backend_outcomes() -> Result<()> {
    let (codex_home, server) = chatgpt_test_context().await?;
    let cases = [
        (
            "request-reset",
            "reset",
            ConsumeAccountRateLimitResetCreditOutcome::Reset,
            2,
        ),
        (
            "request-nothing",
            "nothing_to_reset",
            ConsumeAccountRateLimitResetCreditOutcome::NothingToReset,
            0,
        ),
        (
            "request-no-credit",
            "no_credit",
            ConsumeAccountRateLimitResetCreditOutcome::NoCredit,
            0,
        ),
        (
            "request-retry",
            "already_redeemed",
            ConsumeAccountRateLimitResetCreditOutcome::AlreadyRedeemed,
            0,
        ),
    ];
    for (idempotency_key, backend_code, _, windows_reset) in cases {
        Mock::given(method("POST"))
            .and(path("/api/codex/rate-limit-reset-credits/consume"))
            .and(header("authorization", "Bearer chatgpt-token"))
            .and(header("chatgpt-account-id", "account-123"))
            .and(body_json(json!({ "redeem_request_id": idempotency_key })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "code": backend_code,
                "windows_reset": windows_reset
            })))
            .mount(&server)
            .await;
    }

    let mut mcp = initialized_app_server(codex_home.path()).await?;
    for (idempotency_key, _, expected_outcome, _) in cases {
        assert_eq!(
            consume_reset_credit(&mut mcp, idempotency_key).await?,
            ConsumeAccountRateLimitResetCreditResponse {
                outcome: expected_outcome,
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn consume_account_rate_limit_reset_credit_rejects_empty_idempotency_key() -> Result<()> {
    let (codex_home, _server) = chatgpt_test_context().await?;
    let mut mcp = initialized_app_server(codex_home.path()).await?;

    let request_id = mcp
        .send_consume_account_rate_limit_reset_credit_request(
            ConsumeAccountRateLimitResetCreditParams {
                idempotency_key: String::new(),
            },
        )
        .await?;
    let error = read_error_response(&mut mcp, request_id).await?;

    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "idempotencyKey must not be empty");
    Ok(())
}

#[tokio::test]
async fn consume_account_rate_limit_reset_credit_surfaces_backend_failure() -> Result<()> {
    let (codex_home, server) = chatgpt_test_context().await?;
    Mock::given(method("POST"))
        .and(path("/api/codex/rate-limit-reset-credits/consume"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let mut mcp = initialized_app_server(codex_home.path()).await?;
    let request_id = send_consume_reset_credit(&mut mcp, "request-1").await?;
    let error = read_error_response(&mut mcp, request_id).await?;

    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert!(
        error
            .error
            .message
            .contains("failed to consume rate limit reset"),
        "unexpected error message: {}",
        error.error.message
    );
    Ok(())
}

#[tokio::test]
async fn consume_timeout_releases_account_auth_queue() -> Result<()> {
    let (codex_home, server) = chatgpt_test_context().await?;
    Mock::given(method("POST"))
        .and(path("/api/codex/rate-limit-reset-credits/consume"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(/*secs*/ 1))
                .set_body_json(json!({ "code": "reset", "windows_reset": 2 })),
        )
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("OPENAI_API_KEY", None),
            (RATE_LIMIT_RESET_REQUEST_TIMEOUT_ENV_VAR, Some("100")),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let consume_id = send_consume_reset_credit(&mut mcp, "request-timeout").await?;
    let account_id = mcp
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;

    let consume_error: JSONRPCError = timeout(
        SERVER_TIMEOUT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(consume_id)),
    )
    .await??;
    assert_eq!(consume_error.error.code, INTERNAL_ERROR_CODE);
    assert_eq!(
        consume_error.error.message,
        "rate limit reset consume timed out"
    );

    let account_error = read_error_response(&mut mcp, account_id).await?;
    assert_eq!(account_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        account_error.error.message,
        "email and plan type are required for chatgpt authentication"
    );
    Ok(())
}

async fn chatgpt_test_context() -> Result<(TempDir, MockServer)> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Ok((codex_home, server))
}

async fn initialized_app_server(codex_home: &Path) -> Result<TestAppServer> {
    let mut mcp = TestAppServer::new_with_env(codex_home, &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

async fn consume_reset_credit(
    mcp: &mut TestAppServer,
    idempotency_key: &str,
) -> Result<ConsumeAccountRateLimitResetCreditResponse> {
    let request_id = send_consume_reset_credit(mcp, idempotency_key).await?;
    read_response(mcp, request_id).await
}

async fn send_consume_reset_credit(mcp: &mut TestAppServer, idempotency_key: &str) -> Result<i64> {
    mcp.send_consume_account_rate_limit_reset_credit_request(
        ConsumeAccountRateLimitResetCreditParams {
            idempotency_key: idempotency_key.to_string(),
        },
    )
    .await
}

async fn read_response<T>(mcp: &mut TestAppServer, request_id: i64) -> Result<T>
where
    T: DeserializeOwned,
{
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn read_error_response(mcp: &mut TestAppServer, request_id: i64) -> Result<JSONRPCError> {
    let error = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    Ok(error)
}

async fn login_with_api_key(mcp: &mut TestAppServer, api_key: &str) -> Result<()> {
    let request_id = mcp.send_login_account_api_key_request(api_key).await?;
    assert_eq!(
        read_response::<LoginAccountResponse>(mcp, request_id).await?,
        LoginAccountResponse::ApiKey {}
    );
    Ok(())
}

fn write_chatgpt_base_url(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!("chatgpt_base_url = \"{base_url}\"\n"),
    )
}
