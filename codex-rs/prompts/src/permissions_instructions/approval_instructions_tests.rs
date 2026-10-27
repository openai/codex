//! Verifies catalog heading bounds and preservation of bundled instruction line endings.

#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn replacing_heading_preserves_the_bundled_line_endings() {
    let bundled = "Heading\r\n\r\nRequired instructions.\r\n";
    assert_eq!(
        with_heading(Some("Catalog"), bundled),
        "Catalog\r\n\r\nRequired instructions.\r\n"
    );
}

#[test]
fn accepts_bounded_heading_and_rejects_malformed_presentation() {
    let boundary = "é".repeat(250);
    assert!(valid_heading(&boundary));
    let (open, close) = PermissionsInstructions::type_markers();
    for invalid in [
        "",
        "  ",
        &format!("{boundary}a"),
        open,
        close,
        "{{ policy }}",
        "title\nbody",
        "title\u{2028}body",
    ] {
        assert!(!valid_heading(invalid), "accepted {invalid:?}");
    }
}
