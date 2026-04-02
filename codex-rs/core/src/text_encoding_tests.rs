use super::*;
use encoding_rs::SHIFT_JIS;
use pretty_assertions::assert_eq;

#[test]
fn test_utf8_passthrough() {
    // Fast path: when UTF-8 is valid we should avoid copies and return as-is.
    let utf8_text = "Hello, мир! 世界";
    let bytes = utf8_text.as_bytes();
    assert_eq!(bytes_to_string_smart(bytes), utf8_text);
}

#[test]
fn test_cp1251_russian_text() {
    // Cyrillic text emitted by PowerShell/WSL in CP1251 should decode cleanly.
    let bytes = b"\xEF\xF0\xE8\xEC\xE5\xF0"; // "пример" encoded with Windows-1251
    assert_eq!(bytes_to_string_smart(bytes), "пример");
}

#[test]
fn test_cp1251_privet_word() {
    // Regression: CP1251 words like "Привет" must not be mis-identified as Windows-1252.
    let bytes = b"\xCF\xF0\xE8\xE2\xE5\xF2"; // "Привет" encoded with Windows-1251
    assert_eq!(bytes_to_string_smart(bytes), "Привет");
}

#[test]
fn test_koi8_r_privet_word() {
    // KOI8-R output should decode to the original Cyrillic as well.
    let bytes = b"\xF0\xD2\xC9\xD7\xC5\xD4"; // "Привет" encoded with KOI8-R
    assert_eq!(bytes_to_string_smart(bytes), "Привет");
}

#[test]
fn test_cp866_russian_text() {
    // Legacy consoles (cmd.exe) commonly emit CP866 bytes for Cyrillic content.
    let bytes = b"\xAF\xE0\xA8\xAC\xA5\xE0"; // "пример" encoded with CP866
    assert_eq!(bytes_to_string_smart(bytes), "пример");
}

#[test]
fn test_cp866_uppercase_text() {
    // Ensure the IBM866 heuristic still returns IBM866 for uppercase-only words.
    let bytes = b"\x8F\x90\x88"; // "ПРИ" encoded with CP866 uppercase letters
    assert_eq!(bytes_to_string_smart(bytes), "ПРИ");
}

#[test]
fn test_cp866_uppercase_followed_by_ascii() {
    // Regression test: uppercase CP866 tokens next to ASCII text should not be treated as
    // CP1252.
    let bytes = b"\x8F\x90\x88 test"; // "ПРИ test" encoded with CP866 uppercase letters followed by ASCII
    assert_eq!(bytes_to_string_smart(bytes), "ПРИ test");
}

#[test]
fn test_windows_1252_quotes() {
    // Smart detection should map Windows-1252 punctuation into proper Unicode.
    let bytes = b"\x93\x94test";
    assert_eq!(bytes_to_string_smart(bytes), "\u{201C}\u{201D}test");
}

#[test]
fn test_windows_1252_multiple_quotes() {
    // Longer snippets of punctuation (e.g., “foo” – “bar”) should still flip to CP1252.
    let bytes = b"\x93foo\x94 \x96 \x93bar\x94";
    assert_eq!(
        bytes_to_string_smart(bytes),
        "\u{201C}foo\u{201D} \u{2013} \u{201C}bar\u{201D}"
    );
}

#[test]
fn test_windows_1252_privet_gibberish_is_preserved() {
    // Windows-1252 cannot encode Cyrillic; if the input literally contains "ÐŸÑ..." we should not "fix" it.
    let bytes = "ÐŸÑ€Ð¸Ð²ÐµÑ‚".as_bytes();
    assert_eq!(bytes_to_string_smart(bytes), "ÐŸÑ€Ð¸Ð²ÐµÑ‚");
}

#[test]
fn test_windows_932_shift_jis_text() {
    let (encoded, _, had_errors) = SHIFT_JIS.encode("こんにちは");
    assert!(!had_errors, "failed to encode Shift-JIS sample");
    assert_eq!(bytes_to_string_smart(encoded.as_ref()), "こんにちは");
}

#[test]
fn test_latin1_cafe() {
    // Latin-1 bytes remain common in Western-European locales; decode them directly.
    let bytes = b"caf\xE9"; // codespell:ignore caf
    assert_eq!(bytes_to_string_smart(bytes), "café");
}

#[test]
fn test_preserves_ansi_sequences() {
    // ANSI escape sequences should survive regardless of the detected encoding.
    let bytes = b"\x1b[31mred\x1b[0m";
    assert_eq!(bytes_to_string_smart(bytes), "\x1b[31mred\x1b[0m");
}

#[test]
fn test_fallback_to_lossy() {
    // Completely invalid sequences fall back to the old lossy behavior.
    let invalid_bytes = [0xFF, 0xFE, 0xFD];
    let result = bytes_to_string_smart(&invalid_bytes);
    assert_eq!(result, String::from_utf8_lossy(&invalid_bytes));
}
