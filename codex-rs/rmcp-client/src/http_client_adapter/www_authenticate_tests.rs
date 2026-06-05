use codex_exec_server::HttpHeader;
use pretty_assertions::assert_eq;

use super::BearerInsufficientScope;
use super::InsufficientScopeChallenge;
use super::insufficient_scope_challenge;
use super::is_bearer_invalid_token_challenge;
use super::parse_bearer_insufficient_scope;

#[test]
fn extracts_scope_from_bearer_insufficient_scope_challenges() {
    let cases = [
        (
            r#"Bearer error="insufficient_scope", scope="files:read files:write""#,
            "files:read files:write",
        ),
        (
            r#"Bearer error="insufficient_scope", ScOpE = "files:read""#,
            "files:read",
        ),
        (
            r#"Bearer scope="read:data", error="insufficient_scope""#,
            "read:data",
        ),
        (r#"Bearer error="insufficient_scope", scope=read"#, "read"),
        (
            r#"Bearer error="insufficient_scope", scope="files:read\ files:write""#,
            "files:read files:write",
        ),
        (
            r#"Bearer error="insufficient_scope", error_description="request scope=admin, not \"root\"", scope="files:read""#,
            "files:read",
        ),
        (
            r#"Basic realm="example", Bearer error="insufficient_scope", scope="files:read""#,
            "files:read",
        ),
        (
            r#"Newauth scope="wrong", Bearer error="insufficient_scope", scope="files:read""#,
            "files:read",
        ),
    ];

    for (header, expected_scope) in cases {
        assert_eq!(
            parse_bearer_insufficient_scope(header),
            Some(BearerInsufficientScope {
                required_scope: Some(expected_scope.to_string()),
            }),
            "header: {header}"
        );
    }
}

#[test]
fn does_not_treat_other_bearer_errors_as_insufficient_scope() {
    assert_eq!(
        parse_bearer_insufficient_scope(r#"Bearer error="invalid_token", scope="files:read""#),
        None
    );
}

#[test]
fn rejects_invalid_or_ambiguous_scope_parameters() {
    let cases = [
        r#"Bearer error="insufficient_scope", scope="#,
        r#"Bearer error="insufficient_scope", scope="read\"write""#,
        r#"Bearer error="insufficient_scope", scope="read\\write""#,
        r#"Bearer error="insufficient_scope", scope="read  write""#,
        r#"Bearer error="insufficient_scope", scope=read:data"#,
        r#"Bearer error="insufficient_scope", scope=files:read files:write"#,
        r#"Bearer error="insufficient_scope", scope=read=value"#,
        r#"Bearer error="insufficient_scope", scope="read", scope="write""#,
    ];

    for header in cases {
        assert_eq!(
            parse_bearer_insufficient_scope(header),
            Some(BearerInsufficientScope {
                required_scope: None,
            }),
            "header: {header}"
        );
    }
}

#[test]
fn ignores_scope_text_outside_a_scope_parameter() {
    let cases = [
        r#"Bearer error_description="request scope=admin""#,
        r#"Bearer resource_scope="admin""#,
        r#"Bearer "scope=admin""#,
        r#"Bearer error_description="unterminated scope=admin"#,
    ];

    for header in cases {
        assert_eq!(
            parse_bearer_insufficient_scope(header),
            None,
            "header: {header}"
        );
    }
}

#[test]
fn selects_bearer_challenge_from_a_later_www_authenticate_field_value() {
    let headers = vec![
        HttpHeader {
            name: "www-authenticate".to_string(),
            value: r#"Basic realm="example""#.to_string(),
        },
        HttpHeader {
            name: "WWW-Authenticate".to_string(),
            value: r#"Bearer error="insufficient_scope", scope="files:read""#.to_string(),
        },
    ];

    assert_eq!(
        insufficient_scope_challenge(&headers),
        Some(InsufficientScopeChallenge {
            www_authenticate_header: headers[1].value.clone(),
            required_scope: Some("files:read".to_string()),
        })
    );
}

#[test]
fn detects_only_bearer_invalid_token_challenges() {
    assert!(is_bearer_invalid_token_challenge(
        r#"Basic realm="example", Bearer error="invalid_token""#
    ));

    for value in [
        r#"Bearer realm="example""#,
        r#"Bearer error="insufficient_scope""#,
        r#"Basic error="invalid_token""#,
    ] {
        assert!(!is_bearer_invalid_token_challenge(value), "{value}");
    }
}
