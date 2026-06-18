use bytes::BytesMut;
use pretty_assertions::assert_eq;

use super::LineBuffer;

#[test]
fn searches_only_new_bytes_after_partial_line() {
    let mut buffer = LineBuffer::default();

    buffer.extend_from_slice(b"partial");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
        }
    );

    buffer.extend_from_slice(b" line");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial line"[..]),
            scanned_len: 12,
        }
    );

    buffer.extend_from_slice(b"\nnext");
    assert_eq!(
        buffer.take_line(),
        Some(BytesMut::from(&b"partial line"[..]))
    );
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"next"[..]),
            scanned_len: 0,
        }
    );
}

#[test]
fn splits_multiple_lines_and_retains_partial_tail() {
    let mut buffer = LineBuffer::default();
    buffer.extend_from_slice(b"first\nsecond\npartial");

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"first"[..])));
    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"second"[..])));
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
        }
    );
}

#[test]
fn takes_unterminated_remaining_bytes_at_eof() {
    let mut buffer = LineBuffer::default();
    buffer.extend_from_slice(b"remaining");
    assert_eq!(buffer.take_line(), None);

    assert_eq!(
        buffer.take_remaining(),
        Some(BytesMut::from(&b"remaining"[..]))
    );
    assert_eq!(buffer, LineBuffer::default());
}
