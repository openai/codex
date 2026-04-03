//! Text encoding detection and conversion utilities for shell output.
//!
//! Windows users frequently run into code pages such as CP1251 or CP866 when invoking commands
//! through VS Code. Those bytes show up as invalid UTF-8 and used to be replaced with the standard
//! Unicode replacement character. We now lean on `chardetng` and `encoding_rs` so we can
//! automatically detect and decode the vast majority of legacy encodings before falling back to
//! lossy UTF-8 decoding.

use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use encoding_rs::IBM866;
use encoding_rs::WINDOWS_1252;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StreamOutput<T: Clone> {
    pub text: T,
    pub truncated_after_lines: Option<u32>,
}

impl StreamOutput<String> {
    pub fn new(text: String) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    pub fn from_utf8_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: bytes_to_string_smart(&self.text),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
    pub timed_out: bool,
}

impl Default for ExecToolCallOutput {
    fn default() -> Self {
        Self {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::ZERO,
            timed_out: false,
        }
    }
}

/// Attempts to convert arbitrary bytes to UTF-8 with best-effort encoding detection.
pub fn bytes_to_string_smart(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_owned();
    }

    let encoding = detect_encoding(bytes);
    decode_bytes(bytes, encoding)
}

// Windows-1252 reassigns a handful of 0x80-0x9F slots to smart punctuation (curly quotes, dashes,
// ™). CP866 uses those *same byte values* for uppercase Cyrillic letters. When chardetng sees shell
// snippets that mix these bytes with ASCII it sometimes guesses IBM866, so “smart quotes” render as
// Cyrillic garbage (“УФЦ”) in VS Code. However, CP866 uppercase tokens are perfectly valid output
// (e.g., `ПРИ test`) so we cannot flip every 0x80-0x9F byte to Windows-1252 either. The compromise
// is to only coerce IBM866 to Windows-1252 when (a) the high bytes are exclusively the punctuation
// values listed below and (b) we spot adjacent ASCII. This targets the real failure case without
// clobbering legitimate Cyrillic text. If another code page has a similar collision, introduce a
// dedicated allowlist (like this one) plus unit tests that capture the actual shell output we want
// to preserve. Windows-1252 byte values for smart punctuation.
const WINDOWS_1252_PUNCT_BYTES: [u8; 8] = [
    0x91, // ‘ (left single quotation mark)
    0x92, // ’ (right single quotation mark)
    0x93, // “ (left double quotation mark)
    0x94, // ” (right double quotation mark)
    0x95, // • (bullet)
    0x96, // – (en dash)
    0x97, // — (em dash)
    0x99, // ™ (trade mark sign)
];

fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let (encoding, _is_confident) = detector.guess_assess(None, true);

    // chardetng occasionally reports IBM866 for short strings that only contain Windows-1252 “smart
    // punctuation” bytes (0x80-0x9F) because that range maps to Cyrillic letters in IBM866. When
    // those bytes show up alongside an ASCII word (typical shell output: `"“`test), we know the
    // intent was likely CP1252 quotes/dashes. Prefer WINDOWS_1252 in that specific situation so we
    // render the characters users expect instead of Cyrillic junk. References:
    // - Windows-1252 reserving 0x80-0x9F for curly quotes/dashes:
    //   https://en.wikipedia.org/wiki/Windows-1252
    // - CP866 mapping 0x93/0x94/0x96 to Cyrillic letters, so the same bytes show up as “УФЦ” when
    //   mis-decoded: https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP866.TXT
    if encoding == IBM866 && looks_like_windows_1252_punctuation(bytes) {
        return WINDOWS_1252;
    }

    encoding
}

fn decode_bytes(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (decoded, _, had_errors) = encoding.decode(bytes);

    if had_errors {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    decoded.into_owned()
}

/// Detect whether the byte stream looks like Windows-1252 “smart punctuation” wrapped around
/// otherwise-ASCII text.
///
/// Context: IBM866 and Windows-1252 share the 0x80-0x9F slot range. In IBM866 these bytes decode to
/// Cyrillic letters, whereas Windows-1252 maps them to curly quotes and dashes. chardetng can guess
/// IBM866 for short snippets that only contain those bytes, which turns shell output such as
/// `“test”` into unreadable Cyrillic. To avoid that, we treat inputs comprising a handful of bytes
/// from the problematic range plus ASCII letters as CP1252 punctuation. We deliberately do *not*
/// cap how many of those punctuation bytes we accept: VS Code frequently prints several quoted
/// phrases (e.g., `"foo" - "bar"`), and truncating the count would once again mis-decode those as
/// Cyrillic. If we discover additional encodings with overlapping byte ranges, prefer adding
/// encoding-specific byte allowlists like `WINDOWS_1252_PUNCT` and tests that exercise real-world
/// shell snippets.
fn looks_like_windows_1252_punctuation(bytes: &[u8]) -> bool {
    let mut saw_extended_punctuation = false;
    let mut saw_ascii_word = false;

    for &byte in bytes {
        if byte >= 0xA0 {
            return false;
        }
        if (0x80..=0x9F).contains(&byte) {
            if !is_windows_1252_punct(byte) {
                return false;
            }
            saw_extended_punctuation = true;
        }
        if byte.is_ascii_alphabetic() {
            saw_ascii_word = true;
        }
    }

    saw_extended_punctuation && saw_ascii_word
}

fn is_windows_1252_punct(byte: u8) -> bool {
    WINDOWS_1252_PUNCT_BYTES.contains(&byte)
}

#[cfg(test)]
#[path = "exec_output_tests.rs"]
mod tests;
