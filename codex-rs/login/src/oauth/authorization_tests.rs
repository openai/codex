//! Callback state must match before codes or provider errors can be consumed.

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn callback_validation_checks_state_before_code_or_provider_errors() {
    for (query, expected) in [
        ("state=expected&code=trusted-code", Ok("trusted-code")),
        ("code=untrusted", Err("state")),
        (
            "state=wrong&code=untrusted&error=access_denied",
            Err("state"),
        ),
        ("state=expected.extra&code=untrusted", Err("state")),
        (
            "state=expected&code=ignored&error=access_denied",
            Err("provider"),
        ),
        ("state=expected&code=", Err("code")),
    ] {
        let url = Url::parse(&format!("http://localhost/callback?{query}")).unwrap();
        let params = CallbackParameters::from_url(&url);
        let actual = params.validate("expected").map_err(|error| match error {
            CallbackError::StateMismatch => "state",
            CallbackError::Provider { .. } => "provider",
            CallbackError::MissingCode => "code",
        });
        assert_eq!(actual, expected, "callback query {query}");
    }
}
