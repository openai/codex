//! Credential selection honors dedicated keys and restricts ambient-key fallback.

use super::decisions_api_key;
use pretty_assertions::assert_eq;
use std::env::VarError;

#[test]
fn decisions_key_fallback_preserves_precedence_and_provider_boundary() {
    for (provider, base_url, dedicated, expected) in [
        ("openai", None, None, Some("standard")),
        ("openai", None, Some("dedicated"), Some("dedicated")),
        ("openai", None, Some(""), Some("")),
        ("custom", None, None, None),
        ("openai", Some("https://gateway.example/v1"), None, None),
        ("custom", None, Some("dedicated"), Some("dedicated")),
    ] {
        let result = decisions_api_key(provider, base_url, |name| match name {
            "CODEX_GUARDIAN_DECISIONS_API_KEY" => {
                dedicated.map(str::to_owned).ok_or(VarError::NotPresent)
            }
            "OPENAI_API_KEY" => {
                assert_eq!((provider, base_url, dedicated), ("openai", None, None));
                Ok("standard".into())
            }
            _ => panic!("unexpected environment variable: {name}"),
        });
        assert_eq!(
            result,
            expected.map(str::to_owned).ok_or(VarError::NotPresent)
        );
    }
}
