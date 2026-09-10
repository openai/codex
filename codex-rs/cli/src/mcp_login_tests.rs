//! Checks bounded callback input independently of terminal state.

use super::MAX_CALLBACK_BYTES;
use super::read_callback_line;
use pretty_assertions::assert_eq;
use std::io::Cursor;

#[test]
fn callback_line_accepts_crlf_and_eof_after_input() {
    for input in [
        "https://example.test/callback?code=abc\r\n",
        "https://example.test/callback?code=abc",
    ] {
        assert_eq!(
            read_callback_line(Cursor::new(input)).expect("read callback URL"),
            "https://example.test/callback?code=abc"
        );
    }
}

#[test]
fn callback_line_is_bounded_and_does_not_expose_input_in_errors() {
    let maximum_input = "s".repeat(MAX_CALLBACK_BYTES);
    assert_eq!(
        read_callback_line(Cursor::new(format!("{maximum_input}\r\n"))).expect("bounded input"),
        maximum_input
    );
    let input = "s".repeat(MAX_CALLBACK_BYTES + 1);
    assert_eq!(
        read_callback_line(Cursor::new(input))
            .unwrap_err()
            .to_string(),
        "OAuth callback URL exceeds 64 KiB"
    );
    assert_eq!(
        read_callback_line(Cursor::new("")).unwrap_err().to_string(),
        "No OAuth callback URL received before input closed"
    );
    assert_eq!(
        format!(
            "{:?}",
            read_callback_line(Cursor::new(b"sensitive\xff")).unwrap_err()
        ),
        "OAuth callback URL must be valid UTF-8"
    );
}
