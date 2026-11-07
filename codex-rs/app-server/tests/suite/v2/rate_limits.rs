use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::AddCreditsNudgeEmailStatus;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::GetAccountResponse;
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
use test_case::test_case;
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

#[test_case("enterprise_cbp_automation", AccountPlanType::EnterpriseCbpAutomation; "enterprise_automation")]
#[test_case("promax", AccountPlanType::ProMax; "pro_max")]
#[test_case("edu_plus", AccountPlanType::EduPlus; "edu_plus")]
#[test_case("edu_pro", AccountPlanType::EduPro; "edu_pro")]
#[test_case("sci", AccountPlanType::Sci; "sci")]
#[test_case("sci_plus", AccountPlanType::SciPlus; "sci_plus")]
#[tokio::test]
async fn get_account_rate_limits_returns_snapshot(
    plan_type: &str,
    expected_plan: AccountPlanType,
) -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    let server_url = server.uri();
    let startup_server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &startup_server.uri())?;

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
    let banner = json!({
        "banner_type": "selected_model_limit_reached",
        "title": "Usage limit reached",
        "description": "View your usage.",
        "ctas": [{"action": "view_usage", "label": "View usage"}]
    });
    let response_body = json!({
        "account_id": "account-123",
        "user_id": "user-123",
        "rate_limit_upsell": banner,

        "plan_type": plan_type,
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

    // Account RPCs reload the effective config, but custom endpoints keep their original auth.
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "chatgpt_base_url = {server_url:?}\n[features.runtime_agent_instances]\nenabled = true\nmode = \"assertion_only\"\n",
        ),
    )?;

    let request_id = mcp.send_get_account_rate_limits_request().await?;

    let received: GetAccountRateLimitsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;

    let expected = GetAccountRateLimitsResponse {
        ordinary_usage_allowed: Some(true),
        account_id: Some("account-123".to_string()),
        rate_limit_upsell: Some(banner),
        rate_limits: RateLimitSnapshot {
            limit_id: Some("codex".to_string()),
            limit_name: None,
            normal_model_slug: None,
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
            plan_type: Some(expected_plan),
            rate_limit_reached_type: Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
        },
        rate_limits_by_limit_id: Some(
            [
                (
                    "codex".to_string(),
                    RateLimitSnapshot {
                        limit_id: Some("codex".to_string()),
                        limit_name: None,
                        normal_model_slug: None,
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
                        plan_type: Some(expected_plan),
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
                        normal_model_slug: None,
                        primary: Some(RateLimitWindow {
                            used_percent: 88,
                            window_duration_mins: Some(30),
                            resets_at: Some(1735693200),
                        }),
                        secondary: None,
                        credits: None,
                        individual_limit: None,
                        spend_control_reached: None,
                        plan_type: Some(expected_plan),
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

    for request in server.received_requests().await.expect("recorded requests") {
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    Ok(())
}

#[test_case(Some("workspace-a"), Some("user-a"), /*restricted*/ false, /*permitted*/ true; "matching_identity")]
#[test_case(Some("workspace-b"), Some("user-a"), /*restricted*/ false, /*permitted*/ false; "account_mismatch")]
#[test_case(Some("workspace-a"), Some("user-b"), /*restricted*/ false, /*permitted*/ false; "user_mismatch")]
#[test_case(/*account*/ None, Some("user-a"), /*restricted*/ false, /*permitted*/ false; "missing_account")]
#[test_case(Some("workspace-a"), /*user*/ None, /*restricted*/ false, /*permitted*/ false; "missing_user")]
#[test_case(Some("workspace-a"), Some("user-a"), /*restricted*/ true, /*permitted*/ false; "restricted_account")]
#[tokio::test]
async fn get_account_rate_limits_filters_banner_by_identity(
    account: Option<&str>,
    user: Option<&str>,
    restricted: bool,
    permitted: bool,
) -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut claims = ChatGptIdTokenClaims::new()
        .plan_type("team")
        .chatgpt_user_id("user-a");
    claims.chatgpt_account_is_fedramp = restricted;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("workspace-a")
            .claims(claims),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    let banner = json!({
        "banner_type": "selected_model_limit", "model_slug": "test-model-a",
        "presentation": "inline", "title": "Usage limit reached",
        "description": "Contact your owner.",
        "ctas": [{"action": "notify_owner", "label": "Notify owner"}]
    });
    Mock::given(method("GET"))
        .and(path("/api/codex/usage"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "workspace-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "account_id": account, "user_id": user, "plan_type": "team",
            "rate_limit": {"allowed": true, "limit_reached": false,
                "primary_window": {"used_percent": 42, "limit_window_seconds": 3600,
                    "reset_after_seconds": 120, "reset_at": 2000000000}},
            "rate_limit_upsell": banner, "rate_limit_reset_credits": {"available_count": 0}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/codex/rate-limit-reset-credits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "available_count": 0, "credits": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let request_id = mcp.send_get_account_rate_limits_request().await?;
    let received: GetAccountRateLimitsResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;
    let snapshot = json!({
        "limitId": "codex", "planType": "team",
        "primary": {"usedPercent": 42, "windowDurationMins": 60, "resetsAt": 2000000000}
    });
    let expected: GetAccountRateLimitsResponse = serde_json::from_value(json!({
        "ordinaryUsageAllowed": if permitted { Some(true) } else { None },
        "accountId": account, "rateLimitUpsell": if permitted { Some(banner) } else { None },
        "rateLimits": snapshot, "rateLimitsByLimitId": {"codex": snapshot},
        "rateLimitResetCredits": {"availableCount": 0, "credits": []}
    }))?;
    assert_eq!(received, expected);
    server.verify().await;
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

#[test_case("disabled"; "disabled")]
#[test_case("dual_header"; "dual_header")]
#[test_case("assertion_only"; "assertion_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn account_requests_share_runtime_and_rotate_on_restart(mode: &str) -> Result<()> {
    use base64::Engine;
    use core_test_support::runtime_auth::PROBE_SERVER;
    use core_test_support::runtime_auth::run_harness_through_proxy;
    use std::collections::BTreeSet;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    let enabled = mode != "disabled";
    let token = format!("e30.{}.c2ln", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "exp": chrono::Utc::now().timestamp() + 3600,
            "session_id": "session-a", "iss": "https://auth.openai.com",
            "https://api.openai.com/auth": {"chatgpt_user_id": "user-123", "chatgpt_account_id": "account-123"},
        }))?
    ));
    if std::env::var(PROBE_SERVER).is_ok() {
        let home = TempDir::new()?;
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new(&token)
                .account_id("account-123")
                .chatgpt_user_id("user-123")
                .plan_type("pro"),
            AuthCredentialsStoreMode::File,
        )?;
        let before = std::fs::read(home.path().join("auth.json"))?;
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                "chatgpt_base_url = \"https://chatgpt.com/backend-api\"\nfeatures.connectors = true\n[features.runtime_agent_instances]\nenabled = {enabled}\nmode = {mode:?}\n",
                mode = if enabled { mode } else { "dual_header" },
            ),
        )?;
        for _ in 0..2 {
            let mut app = TestAppServer::builder()
                .with_codex_home(home.path())
                .without_auto_env()
                .with_env_overrides(&[("OPENAI_API_KEY", None)])
                .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
                .await?;
            let id = app
                .send_get_account_request(GetAccountParams {
                    refresh_token: false,
                })
                .await?;
            let account: GetAccountResponse =
                timeout(DEFAULT_READ_TIMEOUT, app.read_response(id)).await??;
            assert_eq!(
                account
                    .workspace_routing
                    .context("workspace routing")?
                    .chatgpt_account_id,
                "account-123"
            );
            for _ in 0..2 {
                let id = app.send_get_account_rate_limits_request().await?;
                let result: GetAccountRateLimitsResponse =
                    timeout(DEFAULT_READ_TIMEOUT, app.read_response(id)).await??;
                assert_eq!(result.rate_limits.plan_type, Some(AccountPlanType::Pro));
                assert_eq!(
                    result
                        .rate_limits
                        .primary
                        .context("primary rate limit")?
                        .used_percent,
                    42
                );
            }
            let id = app
                .send_apps_list_request(codex_app_server_protocol::AppsListParams {
                    limit: None,
                    cursor: None,
                    thread_id: None,
                    force_refetch: true,
                })
                .await?;
            let apps: codex_app_server_protocol::AppsListResponse =
                timeout(DEFAULT_READ_TIMEOUT, app.read_response(id)).await??;
            assert!(!apps.data.is_empty(), "Apps list must return the mock app");
            let id = app
                .send_apps_read_request(codex_app_server_protocol::AppsReadParams {
                    app_ids: vec!["details-only".into()],
                    thread_id: None,
                    include_tools: false,
                })
                .await?;
            let apps: codex_app_server_protocol::AppsReadResponse =
                timeout(DEFAULT_READ_TIMEOUT, app.read_response(id)).await??;
            assert_eq!(apps.apps[0].id, "details-only");
        }
        assert_eq!(std::fs::read(home.path().join("auth.json"))?, before);
        return Ok(());
    }

    let server = MockServer::start().await;
    app_test_support::mount_workspace_routing(&server).await;
    core_test_support::apps_test_server::AppsTestServer::mount(&server).await?;
    Mock::given(method("GET")).and(wiremock::matchers::path_regex(".*/connectors/directory/(list|list_workspace)$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "apps": [{"id": "directory-app", "name": "Directory app", "description": "An app from the directory"}],
            "nextToken": null,
        }))).mount(&server).await;
    Mock::given(method("POST")).and(wiremock::matchers::path_regex(".*/ps/apps/batch$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"apps": [{
            "id": "details-only", "name": "Details", "description": "An app read through the public RPC",
        }]}))).expect(2).mount(&server).await;
    let registrations = AtomicUsize::new(0);
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(move |_: &wiremock::Request| {
            let id = registrations.fetch_add(1, Ordering::Relaxed) + 1;
            ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": format!("runtime-{id}"), "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": "user-123", "account_id": "account-123"},
            }))
        })
        .expect(if enabled { 2 } else { 0 })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/usage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "account_id": "account-123", "user_id": "user-123", "plan_type": "pro",
            "rate_limit": {"allowed": true, "limit_reached": false, "primary_window": {
                "used_percent": 42, "limit_window_seconds": 3600,
                "reset_after_seconds": 120, "reset_at": 1900000000,
            }},
        })))
        .expect(4)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/rate-limit-reset-credits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "credits": [], "available_count": 0, "total_earned_count": 0,
        })))
        .mount(&server)
        .await;
    run_harness_through_proxy(
        &server,
        &format!(
            "suite::v2::rate_limits::account_requests_share_runtime_and_rotate_on_restart::{mode}"
        ),
        /*realtime_target*/ None,
    )
    .await?;

    let requests = server
        .received_requests()
        .await
        .context("captured requests")?;
    let registrations: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/agent/register")
        .collect();
    if enabled {
        let first: serde_json::Value = registrations[0].body_json()?;
        let second: serde_json::Value = registrations[1].body_json()?;
        assert_eq!(first["agent_host_auth"], second["agent_host_auth"]);
        assert_ne!(first["public_key"], second["public_key"]);
        assert_ne!(
            registrations[0].headers["idempotency-key"],
            registrations[1].headers["idempotency-key"]
        );
        for request in registrations {
            assert!(!request.headers.contains_key("authorization"));
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        }
    }
    let mut proofs = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    for request in requests.iter().filter(|request| {
        let path = request.url.path();
        path.starts_with("/backend-api/wham/")
            || path.contains("/connectors/")
            || path.contains("/ps/")
    }) {
        assert_eq!(request.headers["chatgpt-account-id"], "account-123");
        let authorization = request.headers["authorization"].to_str()?;
        let proof = if mode == "assertion_only" {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            authorization
                .strip_prefix("AgentAssertion ")
                .with_context(|| {
                    format!(
                        "assertion-only authentication for {} {}",
                        request.method,
                        request.url.path()
                    )
                })?
        } else {
            assert!(authorization.starts_with("Bearer e30."));
            if !enabled {
                assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                continue;
            }
            request
                .headers
                .get("x-openai-agent-assertion")
                .with_context(|| {
                    format!(
                        "runtime assertion for {} {}",
                        request.method,
                        request.url.path()
                    )
                })?
                .to_str()?
        };
        assert!(
            proofs.insert(proof.to_string()),
            "each request must get a fresh assertion"
        );
        let payload: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).context("assertion payload")?)?,
        )?;
        runtimes.insert(
            payload["runtime_agent_instance"]
                .as_str()
                .context("runtime ID")?
                .to_string(),
        );
    }
    if enabled {
        assert!(proofs.len() >= 4);
        assert_eq!(
            runtimes,
            BTreeSet::from(["runtime-1".to_string(), "runtime-2".to_string()])
        );
    }
    server.verify().await;
    Ok(())
}
