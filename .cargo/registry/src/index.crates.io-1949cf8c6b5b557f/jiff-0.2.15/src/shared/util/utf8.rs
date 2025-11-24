/// Decodes the next UTF-8 encoded codepoint from the given byte slice.
///
/// If no valid encoding of a codepoint exists at the beginning of the
/// given byte slice, then the first byte is returned instead.
///
/// This returns `None` if and only if `bytes` is empty.
///
/// This never panics.
///
/// *WARNING*: This is not designed for performance. If you're looking for
/// a fast UTF-8 decoder, this is not it. If you feel like you need one in
/// this crate, then please file an issue and discuss your use case.
pub(crate) fn decode(bytes: &[u8]) -> Option<Result<char, u8>> {
    /// Given a UTF-8 leading byte, this returns the total number of code
    /// units in the following encoded codepoint.
    ///
    /// If the given byte is not a valid UTF-8 leading byte, then this
    /// returns `None`.
    fn utf8_len(byte: u8) -> Option<usize> {
        if byte <= 0x7F {
            return Some(1);
        } else if byte & 0b1100_0000 == 0b1000_0000 {
            return None;
        } else if byte <= 0b1101_1111 {
            Some(2)
        } else if byte <= 0b1110_1111 {
            Some(3)
        } else if byte <= 0b1111_0111 {
            Some(4)
        } else {
            None
        }
    }

    if bytes.is_empty() {
        return None;
    }
    let len = match utf8_len(bytes[0]) {
        None => return Some(Err(bytes[0])),
        Some(len) if len > bytes.len() => return Some(Err(bytes[0])),
        Some(1) => return Some(Ok(char::from(bytes[0]))),
        Some(len) => len,
    };
    match core::str::from_utf8(&bytes[..len]) {
        Ok(s) => Some(Ok(s.chars().next().unwrap())),
        Err(_) => Some(Err(bytes[0])),
    }
}
