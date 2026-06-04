use super::is_disallowed_osc_text_char;

#[test]
fn rejects_controls_bidi_and_invisible_format_chars() {
    for ch in [
        '\x07', '\x1b', '\u{009b}', '\n', '\u{202E}', '\u{2066}', '\u{200F}', '\u{061C}',
        '\u{200B}', '\u{FEFF}',
    ] {
        assert!(
            is_disallowed_osc_text_char(ch),
            "expected {ch:?} to be disallowed"
        );
    }
}

#[test]
fn allows_ordinary_text() {
    for ch in ['a', 'Z', '0', ' ', '/', '.', '…', 'é'] {
        assert!(
            !is_disallowed_osc_text_char(ch),
            "expected {ch:?} to be allowed"
        );
    }
}
