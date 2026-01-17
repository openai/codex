//! Regression test ensuring ANSI escape sequences are sanitized.
//!
//! `StatusIndicatorWidget` relies on `ansi_escape_line()` to strip raw `\x1b` bytes from rendered
//! output, so this test validates that public contract without asserting on full UI frames.

use codex_ansi_escape::ansi_escape_line;

#[test]
fn ansi_escape_line_strips_escape_sequences() {
    let text_in_ansi_red = "\x1b[31mRED\x1b[0m";

    // The returned line must contain three printable glyphs and **no** raw
    // escape bytes.
    let line = ansi_escape_line(text_in_ansi_red);

    let combined: String = line
        .spans
        .iter()
        .map(|span| span.content.to_string())
        .collect();

    assert_eq!(combined, "RED");
}
