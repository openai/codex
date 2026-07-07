use super::*;
use crate::types::ConsumeRateLimitResetCreditCode;
use crate::types::RateLimitResetCreditDetails;
use crate::types::RateLimitResetCreditsDetails;
use crate::types::RateLimitResetCreditsSummary;
use pretty_assertions::assert_eq;

#[test]
fn rate_limit_reset_contract_uses_expected_paths_and_payloads() {
    assert_eq!(
        test_client("https://example.test", PathStyle::CodexApi).rate_limit_status_url(),
        "https://example.test/api/codex/usage"
    );
    assert_eq!(
        test_client("https://example.test", PathStyle::CodexApi).rate_limit_reset_credits_url(),
        "https://example.test/api/codex/rate-limit-reset-credits"
    );
    assert_eq!(
        test_client("https://example.test", PathStyle::CodexApi)
            .consume_rate_limit_reset_credit_url(),
        "https://example.test/api/codex/rate-limit-reset-credits/consume"
    );
    assert_eq!(
        test_client("https://chatgpt.com/backend-api", PathStyle::ChatGptApi)
            .rate_limit_status_url(),
        "https://chatgpt.com/backend-api/wham/usage"
    );
    assert_eq!(
        test_client("https://chatgpt.com/backend-api", PathStyle::ChatGptApi)
            .rate_limit_reset_credits_url(),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits"
    );
    assert_eq!(
        test_client("https://chatgpt.com/backend-api", PathStyle::ChatGptApi)
            .consume_rate_limit_reset_credit_url(),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"
    );

    assert_eq!(
        serde_json::to_value(ConsumeRateLimitResetCreditRequest {
            redeem_request_id: "redeem-123",
            credit_id: None,
        })
        .unwrap(),
        serde_json::json!({ "redeem_request_id": "redeem-123" })
    );
    assert_eq!(
        serde_json::to_value(ConsumeRateLimitResetCreditRequest {
            redeem_request_id: "redeem-456",
            credit_id: Some("credit-123"),
        })
        .unwrap(),
        serde_json::json!({
            "redeem_request_id": "redeem-456",
            "credit_id": "credit-123",
        })
    );

    let status: RateLimitStatusWithResetCredits = serde_json::from_value(serde_json::json!({
        "plan_type": "plus",
        "rate_limit_reset_credits": { "available_count": 3 }
    }))
    .unwrap();
    assert_eq!(
        status.rate_limit_reset_credits,
        Some(RateLimitResetCreditsSummary { available_count: 3 })
    );

    let details: RateLimitResetCreditsDetails = serde_json::from_value(serde_json::json!({
        "credits": [
            {
                "id": "credit-1",
                "reset_type": "codex_rate_limits",
                "status": "available",
                "granted_at": "2026-06-17T00:00:00Z",
                "expires_at": "2026-07-17T00:00:00Z",
                "redeem_started_at": null,
                "redeemed_at": null,
                "profile_image_url": "https://example.test/avatar.png",
                "profile_user_id": "@friend",
                "title": "Full reset (Weekly + 5 hr)",
                "description": "Ready to redeem"
            },
            {
                "id": "credit-2",
                "reset_type": "codex_rate_limits",
                "status": "available",
                "granted_at": "2026-06-18T00:00:00Z",
                "expires_at": null
            }
        ],
        "available_count": 2,
        "total_earned_count": 4
    }))
    .unwrap();
    assert_eq!(
        details,
        RateLimitResetCreditsDetails {
            credits: vec![
                RateLimitResetCreditDetails {
                    id: "credit-1".to_string(),
                    reset_type: "codex_rate_limits".to_string(),
                    status: "available".to_string(),
                    granted_at: "2026-06-17T00:00:00Z".to_string(),
                    expires_at: Some("2026-07-17T00:00:00Z".to_string()),
                    title: Some("Full reset (Weekly + 5 hr)".to_string()),
                    description: Some("Ready to redeem".to_string()),
                },
                RateLimitResetCreditDetails {
                    id: "credit-2".to_string(),
                    reset_type: "codex_rate_limits".to_string(),
                    status: "available".to_string(),
                    granted_at: "2026-06-18T00:00:00Z".to_string(),
                    expires_at: None,
                    title: None,
                    description: None,
                },
            ],
            available_count: 2,
        }
    );

    let response: ConsumeRateLimitResetCreditResponse = serde_json::from_value(serde_json::json!({
        "code": "reset",
        "credit": { "id": "ignored-by-cli" },
        "windows_reset": 2
    }))
    .unwrap();
    assert_eq!(
        response,
        ConsumeRateLimitResetCreditResponse {
            code: ConsumeRateLimitResetCreditCode::Reset,
            windows_reset: 2,
        }
    );
}

fn test_client(base_url: &str, path_style: PathStyle) -> Client {
    Client {
        base_url: base_url.to_string(),
        http: reqwest::Client::new(),
        auth_provider: codex_model_provider::unauthenticated_auth_provider(),
        user_agent: None,
        chatgpt_account_id: None,
        chatgpt_account_is_fedramp: false,
        path_style,
    }
}
