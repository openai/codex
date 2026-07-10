use bytes::BytesMut;
use pretty_assertions::assert_eq;

use super::LineBuffer;
use super::LineTooLong;
use super::MAX_MCP_STDOUT_LINE_BYTES;

#[test]
fn searches_only_new_bytes_after_partial_line() {
    let mut buffer = LineBuffer::default();

    buffer
        .extend_from_slice(b"partial")
        .expect("partial line should fit");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
            pending_line_bytes: 7,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );

    buffer
        .extend_from_slice(b" line")
        .expect("partial line should fit");
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial line"[..]),
            scanned_len: 12,
            pending_line_bytes: 12,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );

    buffer
        .extend_from_slice(b"\nnext")
        .expect("completed line should fit");
    assert_eq!(
        buffer.take_line(),
        Some(BytesMut::from(&b"partial line"[..]))
    );
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"next"[..]),
            scanned_len: 0,
            pending_line_bytes: 4,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );
}

#[test]
fn splits_multiple_lines_and_retains_partial_tail() {
    let mut buffer = LineBuffer::default();
    buffer
        .extend_from_slice(b"first\nsecond\npartial")
        .expect("lines should fit");

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"first"[..])));
    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"second"[..])));
    assert_eq!(buffer.take_line(), None);
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"partial"[..]),
            scanned_len: 7,
            pending_line_bytes: 7,
            max_line_bytes: MAX_MCP_STDOUT_LINE_BYTES,
        }
    );
}

#[test]
fn takes_unterminated_remaining_bytes_at_eof() {
    let mut buffer = LineBuffer::default();
    buffer
        .extend_from_slice(b"remaining")
        .expect("remaining line should fit");
    assert_eq!(buffer.take_line(), None);

    assert_eq!(
        buffer.take_remaining(),
        Some(BytesMut::from(&b"remaining"[..]))
    );
    assert_eq!(buffer, LineBuffer::default());
}

#[test]
fn rejects_oversized_line_without_retaining_its_prefix() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);
    buffer
        .extend_from_slice(b"12345")
        .expect("line at the limit should fit");
    assert_eq!(buffer.take_line(), None);

    assert_eq!(
        buffer.extend_from_slice(b"6"),
        Err(LineTooLong { max_line_bytes: 5 })
    );
    assert_eq!(buffer, LineBuffer::new(/*max_line_bytes*/ 5));
}

#[test]
fn retains_complete_lines_before_an_oversized_line() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);

    assert_eq!(
        buffer.extend_from_slice(b"first\n123456"),
        Err(LineTooLong { max_line_bytes: 5 })
    );

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"first"[..])));
    assert_eq!(buffer.take_remaining(), None);
}

#[test]
fn accepts_input_larger_than_limit_when_each_line_is_bounded() {
    let mut buffer = LineBuffer::new(/*max_line_bytes*/ 5);

    buffer
        .extend_from_slice(b"12345\nabcde\ntail")
        .expect("each individual line should fit");

    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"12345"[..])));
    assert_eq!(buffer.take_line(), Some(BytesMut::from(&b"abcde"[..])));
    assert_eq!(buffer.take_line(), None);
    assert_eq!(buffer.take_remaining(), Some(BytesMut::from(&b"tail"[..])));
}
