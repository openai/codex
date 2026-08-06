use super::is_safe_for_authorization;
use pretty_assertions::assert_eq;

#[test]
fn accepts_unambiguous_paths() {
    let paths = [
        "/openai/openai",
        "/openai/openai/issues/123",
        "/openai/openai/a..b",
        "/openai/openai/%20space",
        "/openai/openai/%E2%9C%93",
        "/openai/openai/contents/%2Egitignore",
        "/openai/openai/contents/.%2Egithub",
        "/openai/openai/%2e%2efoo",
        "/openai/openai/%2e%2e%2e",
    ];

    assert_eq!(
        paths.map(is_safe_for_authorization),
        [true, true, true, true, true, true, true, true, true]
    );
}

#[test]
fn rejects_paths_with_ambiguous_segments_or_encodings() {
    let paths = [
        "/openai/openai/../codex",
        "/openai/openai/./issues",
        "/openai/openai\\..\\codex",
        "/openai/openai/%2e%2e/codex",
        "/openai/openai/%2E%2E/codex",
        "/openai/openai/%2f..%2fcodex",
        "/openai/openai/%5c..%5ccodex",
        "/openai/openai/%252e%252e/codex",
        "/openai/openai/%",
        "/openai/openai/%2",
        "/openai/openai/%zz",
    ];

    assert_eq!(
        paths.map(is_safe_for_authorization),
        [
            false, false, false, false, false, false, false, false, false, false, false
        ]
    );
}
