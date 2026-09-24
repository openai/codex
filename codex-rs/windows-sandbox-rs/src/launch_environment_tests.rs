//! Select the launch transport using Windows command-line UTF-16 units.

use super::needs_environment;
use pretty_assertions::assert_eq;

#[test]
fn only_large_payloads_use_environment_transport() {
    for (payload, expected) in [
        ("a".repeat(24_000), false),
        ("a".repeat(24_001), true),
        ("🧊".repeat(12_000), false),
        ("🧊".repeat(12_001), true),
    ] {
        assert_eq!(needs_environment(&payload), expected);
    }
}
