/// String utility functions for safe byte boundary operations
///
/// These functions ensure that we don't split strings at invalid UTF-8 boundaries

/// Take at most `max_bytes` from the start of `s`, stopping at a char boundary
pub fn take_bytes_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }

    // Find the last valid UTF-8 boundary at or before max_bytes
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }

    &s[..boundary]
}

/// Take at most `max_bytes` from the end of `s`, stopping at a char boundary
pub fn take_last_bytes_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }

    // Find the first valid UTF-8 boundary at or after (len - max_bytes)
    let start_pos = s.len() - max_bytes;
    let mut boundary = start_pos;
    while boundary < s.len() && !s.is_char_boundary(boundary) {
        boundary += 1;
    }

    &s[boundary..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_take_bytes_at_char_boundary_ascii() {
        let s = "hello world";
        assert_eq!(take_bytes_at_char_boundary(s, 5), "hello");
        assert_eq!(take_bytes_at_char_boundary(s, 100), "hello world");
    }

    #[test]
    fn test_take_bytes_at_char_boundary_utf8() {
        let s = "こんにちは世界";
        // "こ" is 3 bytes in UTF-8
        assert_eq!(take_bytes_at_char_boundary(s, 3), "こ");
        assert_eq!(take_bytes_at_char_boundary(s, 6), "こん");
        // Request 4 bytes but must stop at 3 (char boundary)
        assert_eq!(take_bytes_at_char_boundary(s, 4), "こ");
    }

    #[test]
    fn test_take_last_bytes_at_char_boundary_ascii() {
        let s = "hello world";
        assert_eq!(take_last_bytes_at_char_boundary(s, 5), "world");
        assert_eq!(take_last_bytes_at_char_boundary(s, 100), "hello world");
    }

    #[test]
    fn test_take_last_bytes_at_char_boundary_utf8() {
        let s = "こんにちは世界";
        // "界" is 3 bytes
        assert_eq!(take_last_bytes_at_char_boundary(s, 3), "界");
        assert_eq!(take_last_bytes_at_char_boundary(s, 6), "世界");
        // Request 4 bytes but must start at valid boundary
        assert_eq!(take_last_bytes_at_char_boundary(s, 4), "界");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(take_bytes_at_char_boundary("", 10), "");
        assert_eq!(take_last_bytes_at_char_boundary("", 10), "");
    }
}
