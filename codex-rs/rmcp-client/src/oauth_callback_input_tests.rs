//! Validates pasted callback addresses, parameter ambiguity, and redacted errors.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn pasted_callback_preserves_configured_query_and_decodes_response() {
    let CallbackResult::Success(callback) = parse_callback_url(
        " https://callback.example:9443/registered?tenant=a&code=a%2Bb&state=csrf&iss=https%3A%2F%2Fissuer.example ",
        "https://callback.example:9443/registered?tenant=a",
        "https://issuer.example/authorize?state=csrf",
    )
    .unwrap() else {
        panic!("expected successful callback");
    };
    assert_eq!(
        callback,
        OauthCallbackResult {
            code: "a+b".to_string(),
            state: "csrf".to_string(),
            issuer: Some("https://issuer.example".to_string()),
        }
    );
}

#[test]
fn pasted_callback_rejects_ambiguous_or_unbound_urls_without_echoing_values() {
    let redirect = "http://127.0.0.1:1234/callback/server?tenant=a";
    for input in [
        "http://127.0.0.1:1234/other?tenant=a&code=secret&state=csrf",
        "http://127.0.0.1:1235/callback/server?tenant=a&code=secret&state=csrf",
        "http://127.0.0.1:1234/callback/server?tenant=b&code=secret&state=csrf",
        "http://127.0.0.1:1234/callback/server?tenant=a&tenant=b&code=secret&state=csrf",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&%63ode=second&state=csrf",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&state=csrf&state=second",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&state=csrf&iss=a&iss=b",
        "http://secret@127.0.0.1:1234/callback/server?tenant=a&code=secret&state=csrf",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&state=csrf#fragment",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&state=",
        "http://127.0.0.1:1234/callback/server?tenant=a&code=secret&state=csrf&error=denied",
    ] {
        let error =
            parse_callback_url(input, redirect, "https://issuer.example?state=csrf").unwrap_err();
        assert!(!format!("{error:#}").contains("secret"));
    }
    assert!(parse_callback_url(&"x".repeat(/*n*/ 65_537), redirect, "").is_err());
}

#[test]
fn pasted_provider_error_requires_matching_state_and_redacts_description() {
    let redirect = "http://127.0.0.1:1234/callback/server";
    let input = format!("{redirect}?error=access_denied&error_description=secret&state=csrf");
    let CallbackResult::Error(error) = parse_callback_url(
        &input,
        redirect,
        "https://issuer.example/authorize?state=csrf",
    )
    .unwrap() else {
        panic!("expected provider error");
    };
    assert_eq!(error.to_string(), "OAuth provider returned an error");
    assert!(
        parse_callback_url(
            &input,
            redirect,
            "https://issuer.example/authorize?state=other",
        )
        .is_err()
    );
}
