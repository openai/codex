use super::*;
use crate::types::ConsumeRateLimitResetCreditCode;
use pretty_assertions::assert_eq;

#[test]
fn rate_limit_reset_contract_uses_expected_paths_and_payloads() {
    assert_eq!(
        test_client("https://example.test", PathStyle::CodexApi).rate_limit_status_url(),
        "https://example.test/api/codex/usage"
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
            .consume_rate_limit_reset_credit_url(),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"
    );

    assert_eq!(
        serde_json::to_value(ConsumeRateLimitResetCreditRequest {
            redeem_request_id: "redeem-123",
        })
        .unwrap(),
        serde_json::json!({ "redeem_request_id": "redeem-123" })
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
