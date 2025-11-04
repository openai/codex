//! Text encoding detection and conversion utilities.
//!
//! This module provides improved handling of text encoding for shell command outputs,
//! especially for non-UTF-8 text that commonly appears in Windows/WSL environments.

/// Attempts to convert bytes to UTF-8 string with intelligent encoding detection.
///
/// This function tries multiple encoding strategies:
/// 1. Direct UTF-8 validation (fastest path)
/// 2. Windows-1252/CP1252 decoding (common in Windows environments)
/// 3. ISO-8859-1/Latin-1 decoding (fallback for extended ASCII)
/// 4. Lossy UTF-8 conversion (final fallback)
pub fn bytes_to_string_smart(bytes: &[u8]) -> String {
    // Fast path: check if already valid UTF-8
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_owned();
    }

    // Try Windows-1252 (superset of ISO-8859-1, common in Windows)
    if let Some(decoded) = try_decode_windows_1252(bytes) {
        return decoded;
    }

    // Try ISO-8859-1/Latin-1 as fallback
    if let Some(decoded) = try_decode_latin1(bytes) {
        return decoded;
    }

    // Final fallback: lossy UTF-8 conversion
    String::from_utf8_lossy(bytes).into_owned()
}

/// Attempts to decode bytes as Windows-1252 encoding.
/// Windows-1252 is commonly used in Windows environments and includes
/// characters in the 0x80-0x9F range that are undefined in ISO-8859-1.
fn try_decode_windows_1252(bytes: &[u8]) -> Option<String> {
    // Windows-1252 mapping for 0x80-0x9F range
    const WINDOWS_1252_MAP: [char; 32] = [
        '\u{20AC}', // 0x80 -> EURO SIGN
        '\u{0081}', // 0x81 -> <control>
        '\u{201A}', // 0x82 -> SINGLE LOW-9 QUOTATION MARK
        '\u{0192}', // 0x83 -> LATIN SMALL LETTER F WITH HOOK
        '\u{201E}', // 0x84 -> DOUBLE LOW-9 QUOTATION MARK
        '\u{2026}', // 0x85 -> HORIZONTAL ELLIPSIS
        '\u{2020}', // 0x86 -> DAGGER
        '\u{2021}', // 0x87 -> DOUBLE DAGGER
        '\u{02C6}', // 0x88 -> MODIFIER LETTER CIRCUMFLEX ACCENT
        '\u{2030}', // 0x89 -> PER MILLE SIGN
        '\u{0160}', // 0x8A -> LATIN CAPITAL LETTER S WITH CARON
        '\u{2039}', // 0x8B -> SINGLE LEFT-POINTING ANGLE QUOTATION MARK
        '\u{0152}', // 0x8C -> LATIN CAPITAL LIGATURE OE
        '\u{008D}', // 0x8D -> <control>
        '\u{017D}', // 0x8E -> LATIN CAPITAL LETTER Z WITH CARON
        '\u{008F}', // 0x8F -> <control>
        '\u{0090}', // 0x90 -> <control>
        '\u{2018}', // 0x91 -> LEFT SINGLE QUOTATION MARK
        '\u{2019}', // 0x92 -> RIGHT SINGLE QUOTATION MARK
        '\u{201C}', // 0x93 -> LEFT DOUBLE QUOTATION MARK
        '\u{201D}', // 0x94 -> RIGHT DOUBLE QUOTATION MARK
        '\u{2022}', // 0x95 -> BULLET
        '\u{2013}', // 0x96 -> EN DASH
        '\u{2014}', // 0x97 -> EM DASH
        '\u{02DC}', // 0x98 -> SMALL TILDE
        '\u{2122}', // 0x99 -> TRADE MARK SIGN
        '\u{0161}', // 0x9A -> LATIN SMALL LETTER S WITH CARON
        '\u{203A}', // 0x9B -> SINGLE RIGHT-POINTING ANGLE QUOTATION MARK
        '\u{0153}', // 0x9C -> LATIN SMALL LIGATURE OE
        '\u{009D}', // 0x9D -> <control>
        '\u{017E}', // 0x9E -> LATIN SMALL LETTER Z WITH CARON
        '\u{0178}', // 0x9F -> LATIN CAPITAL LETTER Y WITH DIAERESIS
    ];

    let mut result = String::with_capacity(bytes.len());
    for &byte in bytes {
        let ch = match byte {
            0x00..=0x7F => byte as char,                             // ASCII range
            0x80..=0x9F => WINDOWS_1252_MAP[(byte - 0x80) as usize], // Windows-1252 specific
            0xA0..=0xFF => byte as char,                             // ISO-8859-1 compatible range
        };
        result.push(ch);
    }

    // Validate that the result makes sense (contains reasonable characters)
    if result
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
    {
        return None;
    }

    Some(result)
}

/// Attempts to decode bytes as ISO-8859-1 (Latin-1) encoding.
/// This is a simple 1:1 mapping where each byte maps directly to a Unicode code point.
fn try_decode_latin1(bytes: &[u8]) -> Option<String> {
    let result: String = bytes.iter().map(|&b| b as char).collect();

    // Validate that the result doesn't contain too many control characters
    let control_count = result
        .chars()
        .filter(|c| c.is_control() && *c != '\n' && *c != '\r' && *c != '\t')
        .count();
    let total_chars = result.chars().count();

    // If more than 10% are control characters, this probably isn't Latin-1 text
    if total_chars > 0 && (control_count as f32 / total_chars as f32) > 0.1 {
        return None;
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_passthrough() {
        let utf8_text = "Hello, мир! 世界";
        let bytes = utf8_text.as_bytes();
        assert_eq!(bytes_to_string_smart(bytes), utf8_text);
    }

    #[test]
    fn test_windows_1252_decoding() {
        // Test Windows-1252 specific characters
        let bytes = [0x93, 0x94]; // LEFT and RIGHT DOUBLE QUOTATION MARK
        let result = bytes_to_string_smart(&bytes);
        assert_eq!(result, "\u{201C}\u{201D}"); // " "
    }

    #[test]
    fn test_latin1_decoding() {
        // Test Latin-1 text (like café)
        let bytes = [0x63, 0x61, 0x66, 0xE9]; // "café" in Latin-1
        let result = bytes_to_string_smart(&bytes);
        assert_eq!(result, "café");
    }

    #[test]
    fn test_cyrillic_text() {
        // Example of Russian text that might be encoded in various ways
        let utf8_example = "пример";
        let utf8_bytes = utf8_example.as_bytes();
        assert_eq!(bytes_to_string_smart(utf8_bytes), utf8_example);
    }

    #[test]
    fn test_fallback_to_lossy() {
        // Invalid byte sequences should fall back to lossy conversion
        let invalid_bytes = [0xFF, 0xFE, 0xFD];
        let result = bytes_to_string_smart(&invalid_bytes);
        // Should not panic and should contain some content
        assert!(!result.is_empty());
    }
}
