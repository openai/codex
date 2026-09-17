//! Exercise URL redaction through the public logging boundary.

use super::sanitize_url_for_logging;
use pretty_assertions::assert_eq;

#[test]
fn sanitize_url_for_logging_preserves_only_safe_url_parts() {
    for (url, expected) in [
        (
            "https://user:pass@example.com/oauth/token?code=abc&redirect_uri=http%3A%2F%2Flocalhost%2Fcallback#secret",
            "https://example.com/oauth/token?code=%3Credacted%3E&redirect_uri=http%3A%2F%2Flocalhost%2Fcallback",
        ),
        (
            "https://example.com/base?TOKEN=abc&env=prod",
            "https://example.com/base?TOKEN=%3Credacted%3E&env=prod",
        ),
        ("not a URL", "<invalid-url>"),
    ] {
        assert_eq!(sanitize_url_for_logging(url), expected);
    }
}
