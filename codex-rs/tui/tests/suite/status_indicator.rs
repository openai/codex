//! Regression test: ensure that `StatusIndicatorWidget` sanitises ANSI escape
//! sequences so that no raw `\x1b` bytes are written into the backing
//! buffer.  Rendering logic is tricky to unit‑test end‑to‑end, therefore we
//! verify the *public* contract of `ansi_escape_line()` which the widget now
//! relies on.

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
