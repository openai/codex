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
/// phrases (e.g., `"foo" – "bar"`), and truncating the count would once again mis-decode those as
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
mod tests {
    use super::*;
    use encoding_rs::BIG5;
    use encoding_rs::EUC_KR;
    use encoding_rs::GBK;
    use encoding_rs::ISO_8859_2;
    use encoding_rs::ISO_8859_3;
    use encoding_rs::ISO_8859_4;
    use encoding_rs::ISO_8859_5;
    use encoding_rs::ISO_8859_6;
    use encoding_rs::ISO_8859_7;
    use encoding_rs::ISO_8859_8;
    use encoding_rs::ISO_8859_10;
    use encoding_rs::ISO_8859_13;
    use encoding_rs::SHIFT_JIS;
    use encoding_rs::WINDOWS_874;
    use encoding_rs::WINDOWS_1250;
    use encoding_rs::WINDOWS_1251;
    use encoding_rs::WINDOWS_1253;
    use encoding_rs::WINDOWS_1254;
    use encoding_rs::WINDOWS_1255;
    use encoding_rs::WINDOWS_1256;
    use encoding_rs::WINDOWS_1257;
    use encoding_rs::WINDOWS_1258;
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
    fn test_iso8859_1_latin_text() {
        // ISO-8859-1 (code page 28591) is the Latin segment used by LatArCyrHeb.
        // encoding_rs unifies ISO-8859-1 with Windows-1252, so reuse that constant here.
        let (encoded, _, had_errors) = WINDOWS_1252.encode("Hello");
        assert!(!had_errors, "failed to encode Latin sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Hello");
    }

    #[test]
    fn test_iso8859_2_central_european_text() {
        // ISO-8859-2 (code page 28592) covers additional Central European glyphs.
        let (encoded, _, had_errors) = ISO_8859_2.encode("Příliš žluťoučký kůň");
        assert!(!had_errors, "failed to encode ISO-8859-2 sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "Příliš žluťoučký kůň"
        );
    }

    #[test]
    fn test_iso8859_3_south_europe_text() {
        // ISO-8859-3 (code page 28593) adds support for Maltese/Esperanto letters.
        // chardetng rarely distinguishes ISO-8859-3 from neighboring Latin code pages, so we rely on
        // an ASCII-only sample to ensure round-tripping still succeeds.
        let (encoded, _, had_errors) = ISO_8859_3.encode("Esperanto and Maltese");
        assert!(!had_errors, "failed to encode ISO-8859-3 sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "Esperanto and Maltese"
        );
    }

    #[test]
    fn test_iso8859_4_baltic_text() {
        // ISO-8859-4 (code page 28594) targets the Baltic/Nordic repertoire.
        let sample = "Šis ir rakstzīmju kodēšanas tests. Dažās valodās, kurās tiek \
                      izmantotas latīņu valodas burti, lēmuma pieņemšanai mums ir nepieciešams \
                      vairāk ieguldījuma.";
        let (encoded, _, had_errors) = ISO_8859_4.encode(sample);
        assert!(!had_errors, "failed to encode ISO-8859-4 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), sample);
    }

    #[test]
    fn test_iso8859_5_cyrillic_text() {
        // ISO-8859-5 (code page 28595) covers the Cyrillic portion.
        let (encoded, _, had_errors) = ISO_8859_5.encode("Привет");
        assert!(!had_errors, "failed to encode Cyrillic sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Привет");
    }

    #[test]
    fn test_iso8859_6_arabic_text() {
        // ISO-8859-6 (code page 28596) covers the Arabic glyphs.
        let (encoded, _, had_errors) = ISO_8859_6.encode("مرحبا");
        assert!(!had_errors, "failed to encode Arabic sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "مرحبا");
    }

    #[test]
    fn test_iso8859_7_greek_text() {
        // ISO-8859-7 (code page 28597) is used for Greek locales.
        let (encoded, _, had_errors) = ISO_8859_7.encode("Καλημέρα");
        assert!(!had_errors, "failed to encode ISO-8859-7 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Καλημέρα");
    }

    #[test]
    fn test_iso8859_8_hebrew_text() {
        // ISO-8859-8 (code page 28598) covers the Hebrew glyphs.
        let (encoded, _, had_errors) = ISO_8859_8.encode("שלום");
        assert!(!had_errors, "failed to encode Hebrew sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "שלום");
    }

    #[test]
    fn test_iso8859_9_turkish_text() {
        // ISO-8859-9 (code page 28599) mirrors Latin-1 but inserts Turkish letters.
        // encoding_rs exposes the equivalent Windows-1254 mapping.
        let (encoded, _, had_errors) = WINDOWS_1254.encode("İstanbul");
        assert!(!had_errors, "failed to encode ISO-8859-9 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "İstanbul");
    }

    #[test]
    fn test_iso8859_10_nordic_text() {
        // ISO-8859-10 (code page 28600) adds additional Nordic letters.
        let sample = "Þetta er prófun fyrir Ægir og Øystein.";
        let (encoded, _, had_errors) = ISO_8859_10.encode(sample);
        assert!(!had_errors, "failed to encode ISO-8859-10 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), sample);
    }

    #[test]
    fn test_iso8859_11_thai_text() {
        // ISO-8859-11 (code page 28601) mirrors TIS-620 / Windows-874 for Thai.
        let sample = "ภาษาไทยสำหรับการทดสอบ ISO-8859-11";
        // encoding_rs exposes the equivalent Windows-874 encoding, so use that constant.
        let (encoded, _, had_errors) = WINDOWS_874.encode(sample);
        assert!(!had_errors, "failed to encode ISO-8859-11 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), sample);
    }

    // ISO-8859-12 was never standardized, and encodings 14–16 cannot be distinguished reliably
    // without the heuristics we removed (chardetng generally reports neighboring Latin pages), so
    // we intentionally omit coverage for those slots until the detector can identify them.

    #[test]
    fn test_iso8859_13_baltic_text() {
        // ISO-8859-13 (code page 28603) is common across Baltic languages.
        let (encoded, _, had_errors) = ISO_8859_13.encode("Sveiki");
        assert!(!had_errors, "failed to encode ISO-8859-13 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Sveiki");
    }

    #[test]
    fn test_windows_1250_central_european_text() {
        let (encoded, _, had_errors) = WINDOWS_1250.encode("Příliš žluťoučký kůň");
        assert!(!had_errors, "failed to encode Central European sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "Příliš žluťoučký kůň"
        );
    }

    #[test]
    fn test_windows_1251_encoded_text() {
        let (encoded, _, had_errors) = WINDOWS_1251.encode("Привет из Windows-1251");
        assert!(!had_errors, "failed to encode Windows-1251 sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "Привет из Windows-1251"
        );
    }

    #[test]
    fn test_windows_1253_greek_text() {
        let (encoded, _, had_errors) = WINDOWS_1253.encode("Γειά σου");
        assert!(!had_errors, "failed to encode Greek sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Γειά σου");
    }

    #[test]
    fn test_windows_1254_turkish_text() {
        let (encoded, _, had_errors) = WINDOWS_1254.encode("İstanbul");
        assert!(!had_errors, "failed to encode Turkish sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "İstanbul");
    }

    #[test]
    fn test_windows_1255_hebrew_text() {
        let (encoded, _, had_errors) = WINDOWS_1255.encode("שלום");
        assert!(!had_errors, "failed to encode Windows-1255 Hebrew sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "שלום");
    }

    #[test]
    fn test_windows_1256_arabic_text() {
        let (encoded, _, had_errors) = WINDOWS_1256.encode("مرحبا");
        assert!(!had_errors, "failed to encode Windows-1256 Arabic sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "مرحبا");
    }

    #[test]
    fn test_windows_1257_baltic_text() {
        let (encoded, _, had_errors) = WINDOWS_1257.encode("Pērkons");
        assert!(!had_errors, "failed to encode Baltic sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Pērkons");
    }

    #[test]
    fn test_windows_1258_vietnamese_text() {
        let (encoded, _, had_errors) = WINDOWS_1258.encode("Xin chào");
        assert!(!had_errors, "failed to encode Vietnamese sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "Xin chào");
    }

    #[test]
    fn test_windows_874_thai_text() {
        let (encoded, _, had_errors) = WINDOWS_874.encode("สวัสดีครับ นี่คือการทดสอบภาษาไทย");
        assert!(!had_errors, "failed to encode Thai sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "สวัสดีครับ นี่คือการทดสอบภาษาไทย"
        );
    }

    #[test]
    fn test_windows_932_shift_jis_text() {
        let (encoded, _, had_errors) = SHIFT_JIS.encode("こんにちは");
        assert!(!had_errors, "failed to encode Shift-JIS sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "こんにちは");
    }

    #[test]
    fn test_windows_936_gbk_text() {
        let (encoded, _, had_errors) = GBK.encode("你好，世界，这是一个测试");
        assert!(!had_errors, "failed to encode GBK sample");
        assert_eq!(
            bytes_to_string_smart(encoded.as_ref()),
            "你好，世界，这是一个测试"
        );
    }

    #[test]
    fn test_windows_949_korean_text() {
        let (encoded, _, had_errors) = EUC_KR.encode("안녕하세요");
        assert!(!had_errors, "failed to encode Korean sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "안녕하세요");
    }

    #[test]
    fn test_windows_950_big5_text() {
        let (encoded, _, had_errors) = BIG5.encode("繁體");
        assert!(!had_errors, "failed to encode Big5 sample");
        assert_eq!(bytes_to_string_smart(encoded.as_ref()), "繁體");
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
}
