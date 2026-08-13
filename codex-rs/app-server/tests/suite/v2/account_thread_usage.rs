use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::AccountTokenUsageSummary;
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
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

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(/*secs*/ 30);

#[tokio::test]
async fn account_thread_usage_uses_active_workspace_and_canonical_thread_ids() -> Result<()> {
    let thread_id = "019fc8ab-1fb2-7000-8000-000000000123";
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("chatgpt_base_url = \"{}\"\n", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("active-token").account_id("active-workspace"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("different-token").account_id("different-workspace"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .and(header("authorization", "Bearer active-token"))
        .and(header("chatgpt-account-id", "active-workspace"))
        .and(body_json(json!({ "thread_ids": [thread_id] })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [{
                "thread_id": thread_id,
                "estimated_usage_credits_micros": 46_000_000,
                "estimated_usage_usd_micros": null,
                "groups": [{
                    "model": "gpt-5.4",
                    "reasoning_effort": "high",
                    "speed": "fast",
                    "estimated_usage_credits_micros": 46_000_000,
                    "net_new_input_tokens": 80,
                    "cached_input_tokens": 20,
                    "input_tokens": 100,
                    "output_tokens": 40,
                    "total_tokens": 140
                }]
            }]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;

    let request_id = app_server
        .send_raw_request(
            "account/usage/read",
            Some(json!({ "threadId": "019FC8AB-1FB2-7000-8000-000000000123" })),
        )
        .await?;
    let response: GetAccountTokenUsageResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;

    assert_eq!(
        response,
        GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: None,
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: None,
            thread_usage: Some(ThreadUsage {
                thread_id: thread_id.to_string(),
                estimated_usage_credits_micros: 46_000_000,
                estimated_usage_usd_micros: None,
                groups: vec![ThreadUsageBreakdownGroup {
                    model: Some("gpt-5.4".to_string()),
                    reasoning_effort: Some("high".to_string()),
                    speed: Some("fast".to_string()),
                    estimated_usage_credits_micros: 46_000_000,
                    net_new_input_tokens: Some(80),
                    cached_input_tokens: Some(20),
                    input_tokens: Some(100),
                    output_tokens: Some(40),
                    total_tokens: Some(140),
                }],
            }),
        }
    );
    Ok(())
}

#[tokio::test]
async fn account_thread_usage_supports_externally_managed_authentication() -> Result<()> {
    let thread_id = "019fc8ab-1fb2-7000-8000-000000000456";
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("chatgpt_base_url = \"{}\"\n", server.uri()),
    )?;
    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("external@example.com")
            .plan_type("business")
            .chatgpt_account_id("external-workspace"),
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let login_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            access_token.clone(),
            "external-workspace".to_string(),
            Some("business".to_string()),
        )
        .await?;
    let login: LoginAccountResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(login_id)).await??;
    assert_eq!(login, LoginAccountResponse::ChatgptAuthTokens {});

    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .and(header("authorization", format!("Bearer {access_token}")))
        .and(header("chatgpt-account-id", "external-workspace"))
        .and(body_json(json!({ "thread_ids": [thread_id] })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "threads": [{
                "thread_id": thread_id,
                "estimated_usage_credits_micros": 21_000_000,
                "estimated_usage_usd_micros": 840_000
            }]
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;

    let request_id = app_server
        .send_raw_request("account/usage/read", Some(json!({ "threadId": thread_id })))
        .await?;
    let response: GetAccountTokenUsageResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(
        response.thread_usage,
        Some(ThreadUsage {
            thread_id: thread_id.to_string(),
            estimated_usage_credits_micros: 21_000_000,
            estimated_usage_usd_micros: Some(840_000),
            groups: Vec::new(),
        })
    );
    Ok(())
}

#[tokio::test]
async fn account_thread_usage_hides_unavailable_billing_routes() -> Result<()> {
    let thread_id = "019fc8ab-1fb2-7000-8000-000000000789";
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("chatgpt_base_url = \"{}\"\n", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("active-token").account_id("active-workspace"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .respond_with(ResponseTemplate::new(/*s*/ 403))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = app_server
        .send_raw_request("account/usage/read", Some(json!({ "threadId": thread_id })))
        .await?;
    let response: GetAccountTokenUsageResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(response.thread_usage, None);
    Ok(())
}

#[tokio::test]
async fn account_thread_usage_rejects_malformed_thread_ids_before_backend_requests() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("chatgpt_base_url = \"{}\"\n", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("active-token").account_id("active-workspace"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(method("POST"))
        .and(path("/api/codex/usage/thread_usage/query"))
        .respond_with(ResponseTemplate::new(/*s*/ 200))
        .expect(/*r*/ 0)
        .mount(&server)
        .await;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = app_server
        .send_raw_request(
            "account/usage/read",
            Some(json!({ "threadId": "not-a-thread-id" })),
        )
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert!(error.error.message.starts_with("invalid thread id:"));
    Ok(())
}
