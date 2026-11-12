#![cfg(test)]

use super::*;

#[test]
fn invalid_output_does_not_expose_credentials() {
    for output in [
        r#"{"Authorization": "secret", "Other": 123}"#,
        r#"{"bad name": "secret"}"#,
        r#"{"Authorization": "secret\r\nInjected: value"}"#,
        r#"{"Authorization": "secret", "authorization": "other"}"#,
        r#"{"Authorization":"secret","Authorization":"secret"}"#,
        r#"["secret"]"#,
        "secret",
    ] {
        let error = parse_headers(output.as_bytes()).expect_err("invalid headers");
        assert!(!error.to_string().contains("secret"));
    }
    let headers = parse_headers(br#"{"Authorization":"secret"}"#).unwrap();
    assert!(headers["authorization"].is_sensitive());
    assert!(!format!("{headers:?}").contains("secret"));
}
