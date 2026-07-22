use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::AddCreditsNudgeEmailStatus;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RateLimitResetCredit;
use codex_app_server_protocol::RateLimitResetCreditStatus;
use codex_app_server_protocol::RateLimitResetCreditsSummary;
use codex_app_server_protocol::RateLimitResetType;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RateLimitWindow;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendAddCreditsNudgeEmailParams;
use codex_app_server_protocol::SendAddCreditsNudgeEmailResponse;
use codex_app_server_protocol::SpendControlLimitSnapshot;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::account::PlanType as AccountPlanType;
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
const INTERNAL_ERROR_CODE: i64 = -32603;

#[tokio::test]
async fn get_account_rate_limits_requires_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp.send_get_account_rate_limits_request().await?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "codex account authentication required to read rate limits"
    );

    Ok(())
}

#[tokio::test]
async fn get_account_rate_limits_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    login_with_api_key(&mut mcp, "sk-test-key").await?;

    let request_id = mcp.send_get_account_rate_limits_request().await?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "chatgpt authentication required to read rate limits"
    );

    Ok(())
}

#[tokio::test]
async fn get_account_rate_limits_returns_snapshot() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url)?;

    let primary_reset_timestamp = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:02:00Z")
        .expect("parse primary reset timestamp")
        .timestamp();
    let secondary_reset_timestamp = chrono::DateTime::parse_from_rfc3339("2025-01-01T01:00:00Z")
        .expect("parse secondary reset timestamp")
        .timestamp();
    let reset_credit_granted_at = chrono::DateTime::parse_from_rfc3339("2026-06-17T00:00:00Z")
        .expect("parse reset credit grant timestamp")
        .timestamp();
    let reset_credit_expires_at = chrono::DateTime::parse_from_rfc3339("2026-07-17T00:00:00Z")
        .expect("parse reset credit expiry timestamp")
        .timestamp();
    let second_reset_credit_granted_at =
        chrono::DateTime::parse_from_rfc3339("2026-06-18T00:00:00Z")
            .expect("parse second reset credit grant timestamp")
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
        },
        "rate_limit_reached_type": {
            "type": "workspace_member_usage_limit_reached",
        },
        "spend_control": {
            "reached": false,
            "individual_limit": {
                "source": "workspace_spend_controls",
                "limit": "25000",
                "used": "8000",
                "remaining": "17000",
                "used_percent": 32,
                "remaining_percent": 68,
                "reset_after_seconds": 43200,
                "reset_at": secondary_reset_timestamp,
            }
        },
        "additional_rate_limits": [
            {
                "limit_name": "codex_other",
                "metered_feature": "codex_other",
                "rate_limit": {
                    "allowed": true,
                    "limit_reached": false,
                    "primary_window": {
                        "used_percent": 88,
                        "limit_window_seconds": 1800,
                        "reset_after_seconds": 600,
                        "reset_at": 1735693200
                    }
                }
            }
        ],
        "rate_limit_reset_credits": { "available_count": 3 }
    });

    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/codex/rate-limit-reset-credits"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "credits": [
                {
                    "id": "credit-1",
                    "reset_type": "codex_rate_limits",
                    "status": "available",
                    "granted_at": "2026-06-17T00:00:00Z",
                    "expires_at": "2026-07-17T00:00:00Z",
                    "title": "Full reset (Weekly + 5 hr)",
                    "description": "Ready to redeem"
                },
                {
                    "id": "credit-2",
                    "reset_type": "future_reset_type",
                    "status": "future_status",
                    "granted_at": "2026-06-18T00:00:00Z",
                    "expires_at": null
                }
            ],
            "available_count": 2,
            "total_earned_count": 4
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp.send_get_account_rate_limits_request().await?;

    let received: GetAccountRateLimitsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    let expected = GetAccountRateLimitsResponse {
        rate_limits: RateLimitSnapshot {
            limit_id: Some("codex".to_string()),
            limit_name: None,
            primary: Some(RateLimitWindow {
                used_percent: 42,
                window_duration_mins: Some(60),
                resets_at: Some(primary_reset_timestamp),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 5,
                window_duration_mins: Some(1440),
                resets_at: Some(secondary_reset_timestamp),
            }),
            credits: None,
            individual_limit: Some(SpendControlLimitSnapshot {
                limit: "25000".to_string(),
                used: "8000".to_string(),
                remaining_percent: 68,
                resets_at: secondary_reset_timestamp,
            }),
            spend_control_reached: Some(false),
            plan_type: Some(AccountPlanType::Pro),
            rate_limit_reached_type: Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
        },
        rate_limits_by_limit_id: Some(
            [
                (
                    "codex".to_string(),
                    RateLimitSnapshot {
                        limit_id: Some("codex".to_string()),
                        limit_name: None,
                        primary: Some(RateLimitWindow {
                            used_percent: 42,
                            window_duration_mins: Some(60),
                            resets_at: Some(primary_reset_timestamp),
                        }),
                        secondary: Some(RateLimitWindow {
                            used_percent: 5,
                            window_duration_mins: Some(1440),
                            resets_at: Some(secondary_reset_timestamp),
                        }),
                        credits: None,
                        individual_limit: Some(SpendControlLimitSnapshot {
                            limit: "25000".to_string(),
                            used: "8000".to_string(),
                            remaining_percent: 68,
                            resets_at: secondary_reset_timestamp,
                        }),
                        spend_control_reached: Some(false),
                        plan_type: Some(AccountPlanType::Pro),
                        rate_limit_reached_type: Some(
                            RateLimitReachedType::WorkspaceMemberUsageLimitReached,
                        ),
                    },
                ),
                (
                    "codex_other".to_string(),
                    RateLimitSnapshot {
                        limit_id: Some("codex_other".to_string()),
                        limit_name: Some("codex_other".to_string()),
                        primary: Some(RateLimitWindow {
                            used_percent: 88,
                            window_duration_mins: Some(30),
                            resets_at: Some(1735693200),
                        }),
                        secondary: None,
                        credits: None,
                        individual_limit: None,
                        spend_control_reached: None,
                        plan_type: Some(AccountPlanType::Pro),
                        rate_limit_reached_type: None,
                    },
                ),
            ]
            .into_iter()
            .collect(),
        ),
        rate_limit_reset_credits: Some(RateLimitResetCreditsSummary {
            available_count: 2,
            credits: Some(vec![
                RateLimitResetCredit {
                    id: "credit-1".to_string(),
                    reset_type: RateLimitResetType::CodexRateLimits,
                    status: RateLimitResetCreditStatus::Available,
                    granted_at: reset_credit_granted_at,
                    expires_at: Some(reset_credit_expires_at),
                    title: Some("Full reset (Weekly + 5 hr)".to_string()),
                    description: Some("Ready to redeem".to_string()),
                },
                RateLimitResetCredit {
                    id: "credit-2".to_string(),
                    reset_type: RateLimitResetType::Unknown,
                    status: RateLimitResetCreditStatus::Unknown,
                    granted_at: second_reset_credit_granted_at,
                    expires_at: None,
                    title: None,
                    description: None,
                },
            ]),
        }),
    };
    assert_eq!(received, expected);

    Ok(())
}

#[tokio::test]
async fn get_account_rate_limits_preserves_count_when_reset_credit_details_fail() -> Result<()> {
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

    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plan_type": "pro",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 42,
                    "limit_window_seconds": 3600,
                    "reset_after_seconds": 120,
                    "reset_at": 1735689720
                }
            },
            "rate_limit_reset_credits": { "available_count": 3 }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/codex/rate-limit-reset-credits"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp.send_get_account_rate_limits_request().await?;
    let received: GetAccountRateLimitsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(
        received.rate_limit_reset_credits,
        Some(RateLimitResetCreditsSummary {
            available_count: 3,
            credits: None,
        })
    );

    Ok(())
}

#[tokio::test]
async fn send_add_credits_nudge_email_requires_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_add_credits_nudge_email_request(SendAddCreditsNudgeEmailParams {
            credit_type: AddCreditsNudgeCreditType::Credits,
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "codex account authentication required to notify workspace owner"
    );

    Ok(())
}

#[tokio::test]
async fn send_add_credits_nudge_email_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    login_with_api_key(&mut mcp, "sk-test-key").await?;

    let request_id = mcp
        .send_add_credits_nudge_email_request(SendAddCreditsNudgeEmailParams {
            credit_type: AddCreditsNudgeCreditType::UsageLimit,
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "chatgpt authentication required to notify workspace owner"
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore = "covered by Linux and macOS CI")]
#[tokio::test]
async fn send_add_credits_nudge_email_posts_expected_body() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url)?;

    Mock::given(method("POST"))
        .and(path("/api/codex/accounts/send_add_credits_nudge_email"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(wiremock::matchers::body_json(json!({
            "credit_type": "usage_limit",
        })))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_add_credits_nudge_email_request(SendAddCreditsNudgeEmailParams {
            credit_type: AddCreditsNudgeCreditType::UsageLimit,
        })
        .await?;

    let received: SendAddCreditsNudgeEmailResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(received.status, AddCreditsNudgeEmailStatus::Sent);

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore = "covered by Linux and macOS CI")]
#[tokio::test]
async fn send_add_credits_nudge_email_maps_cooldown() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url)?;

    Mock::given(method("POST"))
        .and(path("/api/codex/accounts/send_add_credits_nudge_email"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_add_credits_nudge_email_request(SendAddCreditsNudgeEmailParams {
            credit_type: AddCreditsNudgeCreditType::Credits,
        })
        .await?;

    let received: SendAddCreditsNudgeEmailResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(received.status, AddCreditsNudgeEmailStatus::CooldownActive);

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore = "covered by Linux and macOS CI")]
#[tokio::test]
async fn send_add_credits_nudge_email_surfaces_backend_failure() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    write_chatgpt_base_url(codex_home.path(), &server_url)?;

    Mock::given(method("POST"))
        .and(path("/api/codex/accounts/send_add_credits_nudge_email"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_add_credits_nudge_email_request(SendAddCreditsNudgeEmailParams {
            credit_type: AddCreditsNudgeCreditType::Credits,
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert!(
        error
            .error
            .message
            .contains("failed to notify workspace owner"),
        "unexpected error message: {}",
        error.error.message
    );
    assert_eq!(error.error.data, None);

    Ok(())
}

async fn login_with_api_key(mcp: &mut TestAppServer, api_key: &str) -> Result<()> {
    let request_id = mcp.send_login_account_api_key_request(api_key).await?;
    let login: LoginAccountResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(login, LoginAccountResponse::ApiKey {});

    Ok(())
}

fn write_chatgpt_base_url(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, format!("chatgpt_base_url = \"{base_url}\"\n"))
}
