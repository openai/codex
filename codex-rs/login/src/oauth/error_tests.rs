//! Token error parsing accepts standard OAuth fields and existing issuer error envelopes.

use pretty_assertions::assert_eq;

use super::TokenErrorDetail;

#[test]
fn parses_standard_legacy_and_plain_text_rejections() {
    for (body, code, display) in [
        (
            r#"{"error":"invalid_grant","error_description":"refresh token expired"}"#,
            Some("invalid_grant"),
            "refresh token expired",
        ),
        (
            r#"{"error":{"code":"proxy_auth_required","message":"proxy authentication required"}}"#,
            Some("proxy_auth_required"),
            "proxy authentication required",
        ),
        (
            r#"{"error":"temporarily_unavailable"}"#,
            Some("temporarily_unavailable"),
            "temporarily_unavailable",
        ),
        (
            r#"{"code":"refresh_token_expired"}"#,
            Some("refresh_token_expired"),
            r#"{"code":"refresh_token_expired"}"#,
        ),
        (
            r#"{"code":"configuration_error","message":"Unknown tenant: acme"}"#,
            Some("configuration_error"),
            r#"{"code":"configuration_error","message":"Unknown tenant: acme"}"#,
        ),
        (
            r#"{"code":"configuration_error","message":"top-level","error_description":"description","error":{"message":"nested"}}"#,
            Some("configuration_error"),
            "description",
        ),
        (
            r#"{"code":"configuration_error","message":"top-level","error":{"message":"nested"}}"#,
            Some("configuration_error"),
            "nested",
        ),
        ("service unavailable", None, "service unavailable"),
        ("  ", None, "unknown error"),
    ] {
        let detail = TokenErrorDetail::parse(body, &[]);
        assert_eq!(
            (detail.error_code(), detail.to_string()),
            (code, display.to_string())
        );
    }
}

#[test]
fn redacts_plain_and_json_escaped_credentials() {
    let secret = r#"secret-"refresh\token+&= /%"#;
    let description = format!("{}: {secret}", "x".repeat(/*n*/ 500));
    for body in [
        serde_json::json!({"error": secret}).to_string(),
        serde_json::json!({"error": "temporarily_unavailable", "error_description": description})
            .to_string(),
        serde_json::json!({"unrecognized_field": description}).to_string(),
        serde_json::json!({"code": "configuration_error", "message": description}).to_string(),
    ] {
        let detail = TokenErrorDetail::parse(&body, &[secret]);
        assert!(detail.display_message.contains("[REDACTED]"));
        assert!(!format!("{detail} {detail:?}").contains("secret-"));
    }
}
