use std::fmt;

use sha2::Digest;
use sha2::Sha256;

pub const DEFAULT_BOUNDED_LOG_VALUE_BYTES: usize = 16 * 1024;

pub fn bounded_display<T>(value: &T) -> String
where
    T: fmt::Display + ?Sized,
{
    bounded_str(&value.to_string())
}

pub fn bounded_debug<T>(value: &T) -> String
where
    T: fmt::Debug + ?Sized,
{
    bounded_str(&format!("{value:?}"))
}

pub fn bounded_str(value: &str) -> String {
    bounded_str_with_limit(value, DEFAULT_BOUNDED_LOG_VALUE_BYTES)
}

pub fn bounded_str_with_limit(value: &str, max_bytes: usize) -> String {
    bounded_utf8_bytes(value.as_bytes(), max_bytes)
}

pub fn bounded_bytes_lossy(value: &[u8]) -> String {
    bounded_bytes_lossy_with_limit(value, DEFAULT_BOUNDED_LOG_VALUE_BYTES)
}

pub fn bounded_bytes_lossy_with_limit(value: &[u8], max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return String::from_utf8_lossy(value).into_owned();
    }

    let (prefix, suffix) = split_bytes(value, max_bytes);
    let shown_bytes = prefix.len().saturating_add(suffix.len());
    assemble_bounded_log_value(
        &String::from_utf8_lossy(prefix),
        &String::from_utf8_lossy(suffix),
        value.len(),
        shown_bytes,
        digest_hex(value),
    )
}

fn bounded_utf8_bytes(value: &[u8], max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return String::from_utf8_lossy(value).into_owned();
    }

    let text = String::from_utf8_lossy(value);
    let prefix_len = utf8_prefix_boundary(&text, max_bytes / 2);
    let suffix_start = utf8_suffix_boundary(&text, max_bytes - max_bytes / 2);
    let prefix = &text[..prefix_len];
    let suffix = &text[suffix_start..];
    let shown_bytes = prefix.len().saturating_add(suffix.len());
    assemble_bounded_log_value(prefix, suffix, value.len(), shown_bytes, digest_hex(value))
}

fn split_bytes(value: &[u8], max_bytes: usize) -> (&[u8], &[u8]) {
    let prefix_len = max_bytes / 2;
    let suffix_len = max_bytes - prefix_len;
    let suffix_start = value.len().saturating_sub(suffix_len);
    (&value[..prefix_len], &value[suffix_start..])
}

fn utf8_prefix_boundary(value: &str, max_bytes: usize) -> usize {
    if value.len() <= max_bytes {
        return value.len();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn utf8_suffix_boundary(value: &str, max_bytes: usize) -> usize {
    if value.len() <= max_bytes {
        return 0;
    }
    let mut boundary = value.len().saturating_sub(max_bytes);
    while boundary < value.len() && !value.is_char_boundary(boundary) {
        boundary += 1;
    }
    boundary
}

fn assemble_bounded_log_value(
    prefix: &str,
    suffix: &str,
    original_bytes: usize,
    shown_bytes: usize,
    sha256: String,
) -> String {
    let omitted_bytes = original_bytes.saturating_sub(shown_bytes);
    format!(
        "{prefix}...[truncated: original_bytes={original_bytes} shown_bytes={shown_bytes} omitted_bytes={omitted_bytes} sha256={sha256}]...{suffix}"
    )
}

fn digest_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn bounded_str_returns_short_values_unchanged() {
        assert_eq!("hello", bounded_str_with_limit("hello", /*max_bytes*/ 16));
    }

    #[test]
    fn bounded_str_preserves_prefix_suffix_and_reports_metadata() {
        let value = "abcdefghijklmnop";

        let bounded = bounded_str_with_limit(value, /*max_bytes*/ 8);

        assert!(bounded.starts_with("abcd...[truncated: original_bytes=16 shown_bytes=8"));
        assert!(bounded.contains(" omitted_bytes=8 "));
        assert!(bounded.contains(" sha256="));
        assert!(bounded.ends_with("]...mnop"));
    }

    #[test]
    fn bounded_str_respects_utf8_boundaries() {
        let value = "αβγδεζηθ";

        let bounded = bounded_str_with_limit(value, /*max_bytes*/ 9);

        assert!(bounded.starts_with("αβ...[truncated:"));
        assert!(bounded.ends_with("]...ηθ"));
    }

    #[test]
    fn bounded_bytes_lossy_hashes_original_bytes() {
        let value = b"abcdef\xffghijklmnop";

        let bounded = bounded_bytes_lossy_with_limit(value, /*max_bytes*/ 8);

        assert!(bounded.starts_with("abcd...[truncated: original_bytes=17 shown_bytes=8"));
        assert!(bounded.ends_with("]...mnop"));
        assert!(bounded.contains(&digest_hex(value)));
    }

    #[test]
    fn bounded_debug_formats_then_bounds() {
        let value = Some("a".repeat(DEFAULT_BOUNDED_LOG_VALUE_BYTES));

        let bounded = bounded_debug(&value);

        assert!(bounded.starts_with("Some(\""));
        assert!(bounded.contains("[truncated:"));
    }
}
