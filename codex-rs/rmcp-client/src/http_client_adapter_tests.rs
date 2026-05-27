use pretty_assertions::assert_eq;

use super::extract_scope_from_www_authenticate;

#[test]
fn extracts_scope_authentication_parameters() {
    let cases = [
        (
            r#"Bearer error="insufficient_scope", scope="files:read files:write""#,
            "files:read files:write",
        ),
        (
            r#"Bearer scope=read:data, error="insufficient_scope""#,
            "read:data",
        ),
        (
            r#"Bearer error="insufficient_scope", ScOpE = "files:read""#,
            "files:read",
        ),
    ];

    for (header, expected_scope) in cases {
        assert_eq!(
            extract_scope_from_www_authenticate(header),
            Some(expected_scope.to_string()),
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
        r#"Bearer error="insufficient_scope", scope="#,
        r#"Bearer error_description="unterminated scope=admin"#,
    ];

    for header in cases {
        assert_eq!(
            extract_scope_from_www_authenticate(header),
            None,
            "header: {header}"
        );
    }
}

#[test]
fn skips_scope_text_in_quoted_values_before_the_scope_parameter() {
    assert_eq!(
        extract_scope_from_www_authenticate(
            r#"Bearer error_description="request scope=admin, not \"root\"", scope="files:read""#
        ),
        Some("files:read".to_string())
    );
}
