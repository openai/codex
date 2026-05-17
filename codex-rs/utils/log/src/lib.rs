use std::fmt;

use sha2::Digest;
use sha2::Sha256;

pub const DEFAULT_BOUNDED_LOG_VALUE_BYTES: usize = 16 * 1024;

pub fn bounded_display<T>(value: &T) -> String
where
    T: fmt::Display + ?Sized,
{
    bounded_format_with_limit(format_args!("{value}"), DEFAULT_BOUNDED_LOG_VALUE_BYTES)
}

pub fn bounded_debug<T>(value: &T) -> String
where
    T: fmt::Debug + ?Sized,
{
    bounded_format_with_limit(format_args!("{value:?}"), DEFAULT_BOUNDED_LOG_VALUE_BYTES)
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

fn bounded_format_with_limit(args: fmt::Arguments<'_>, max_bytes: usize) -> String {
    let mut writer = BoundedFormatWriter::new(max_bytes);
    if fmt::write(&mut writer, args).is_err() {
        return String::from("<failed to format bounded log value>");
    }
    writer.finish()
}

struct BoundedFormatWriter {
    max_bytes: usize,
    head: String,
    tail: String,
    full: Option<String>,
    original_bytes: usize,
    digest: Sha256,
}

impl BoundedFormatWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            head: String::new(),
            tail: String::new(),
            full: Some(String::new()),
            original_bytes: 0,
            digest: Sha256::new(),
        }
    }

    fn finish(self) -> String {
        if let Some(full) = self.full {
            return full;
        }

        let shown_bytes = self.head.len().saturating_add(self.tail.len());
        let digest = self.digest.finalize();
        assemble_bounded_log_value(
            &self.head,
            &self.tail,
            self.original_bytes,
            shown_bytes,
            format!("{digest:x}"),
        )
    }

    fn head_capacity(&self) -> usize {
        self.max_bytes / 2
    }

    fn tail_capacity(&self) -> usize {
        self.max_bytes - self.head_capacity()
    }

    fn push_head(&mut self, value: &str) {
        let remaining = self.head_capacity().saturating_sub(self.head.len());
        if remaining == 0 {
            return;
        }

        let boundary = utf8_prefix_boundary(value, remaining);
        self.head.push_str(&value[..boundary]);
    }

    fn push_tail(&mut self, value: &str) {
        let capacity = self.tail_capacity();
        if capacity == 0 {
            return;
        }

        if value.len() >= capacity {
            let start = utf8_suffix_boundary(value, capacity);
            self.tail.clear();
            self.tail.push_str(&value[start..]);
            return;
        }

        self.tail.push_str(value);
        if self.tail.len() > capacity {
            let start = utf8_suffix_boundary(&self.tail, capacity);
            self.tail.replace_range(..start, "");
        }
    }
}

impl fmt::Write for BoundedFormatWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let original_bytes = self.original_bytes.saturating_add(value.len());
        self.digest.update(value.as_bytes());
        self.push_head(value);
        self.push_tail(value);

        if let Some(full) = &mut self.full {
            if original_bytes <= self.max_bytes {
                full.push_str(value);
            } else {
                self.full = None;
            }
        }

        self.original_bytes = original_bytes;
        Ok(())
    }
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

    #[test]
    fn bounded_format_streams_prefix_suffix_and_hash() {
        struct ChunkedDisplay;

        impl fmt::Display for ChunkedDisplay {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for chunk in ["abcd", "efgh", "ijkl", "mnop"] {
                    f.write_str(chunk)?;
                }
                Ok(())
            }
        }

        let bounded =
            bounded_format_with_limit(format_args!("{ChunkedDisplay}"), /*max_bytes*/ 8);

        assert!(bounded.starts_with("abcd...[truncated: original_bytes=16 shown_bytes=8"));
        assert!(bounded.ends_with("]...mnop"));
        assert!(bounded.contains(&digest_hex(b"abcdefghijklmnop")));
    }
}
