use super::*;
use pretty_assertions::assert_eq;

#[test]
fn removes_reported_keyboard_escape_sequence() {
    assert_eq!(
        sanitize_untrusted_text("_count_r\x1b[13;2:3uows"),
        "_count_rows"
    );
}

#[test]
fn preserves_paste_whitespace_and_removes_other_controls() {
    assert_eq!(
        sanitize_untrusted_text("one\tindent\n\0two\u{7f}\n"),
        "one\tindent\ntwo\n"
    );
}
