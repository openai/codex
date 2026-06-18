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
        Some(BytesMut::from(&b"partial line\n"[..]))
    );
    assert_eq!(
        buffer,
        LineBuffer {
            bytes: BytesMut::from(&b"next"[..]),
            scanned_len: 0,
        }
    );
}
