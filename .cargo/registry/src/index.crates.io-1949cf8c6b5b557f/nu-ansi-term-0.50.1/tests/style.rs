use nu_ansi_term::Style;

#[test]
fn manual_instance_style() {
    let s = Style { ..Style::default() };
    assert_eq!(Style::default(), s);

    let s = Style {
        is_underline: false,
        ..Style::default()
    };
    assert_eq!(Style::default(), s);

    let s = Style {
        foreground: None,
        background: None,
        is_bold: false,
        is_dimmed: false,
        is_italic: false,
        is_underline: false,
        is_blink: false,
        is_reverse: false,
        is_hidden: false,
        is_strikethrough: false,
        prefix_with_reset: false,
    };
    assert_eq!(Style::default(), s);
}
