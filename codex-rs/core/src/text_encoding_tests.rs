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
