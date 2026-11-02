//! Unit tests for the bearer_auth_provider module.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn bearer_auth_provider_reports_when_auth_header_will_attach() {
    let auth = BearerAuthProvider {
        token: Some("access-token".to_string()),
        account_id: None,
        is_fedramp_account: false,
    };

    assert_eq!(
        codex_api::auth_header_telemetry(&auth).unwrap(),
        codex_api::AuthHeaderTelemetry {
            attached: true,
            name: Some("authorization"),
        }
    );
}

#[test]
fn bearer_auth_provider_adds_auth_headers() {
    let auth = BearerAuthProvider::for_test(Some("access-token"), Some("workspace-123"));
    let mut headers = HeaderMap::new();

    auth.add_auth_headers(&mut headers).unwrap();

    assert_eq!(
        headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer access-token")
    );
    assert_eq!(
        headers
            .get("ChatGPT-Account-ID")
            .and_then(|value| value.to_str().ok()),
        Some("workspace-123")
    );
}

#[test]
fn bearer_auth_provider_adds_fedramp_routing_header_for_fedramp_accounts() {
    let auth = BearerAuthProvider {
        token: Some("access-token".to_string()),
        account_id: Some("workspace-123".to_string()),
        is_fedramp_account: true,
    };
    let mut headers = HeaderMap::new();

    auth.add_auth_headers(&mut headers).unwrap();

    assert_eq!(
        headers
            .get("X-OpenAI-Fedramp")
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
}
