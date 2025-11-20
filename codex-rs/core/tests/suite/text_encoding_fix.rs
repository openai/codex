//! Integration test for the text encoding fix for issue #6178.
//!
//! These tests simulate VSCode's shell preview on Windows/WSL where the output
//! may be encoded with a legacy code page before it reaches Codex.

use codex_core::exec::StreamOutput;
use pretty_assertions::assert_eq;

#[test]
fn test_utf8_shell_output() {
    // Baseline: UTF-8 output should bypass the detector and remain unchanged.
    assert_eq!(decode_shell_output("пример".as_bytes()), "пример");
}

#[test]
fn test_cp1251_shell_output() {
    // VS Code shells on Windows frequently surface CP1251 bytes for Cyrillic text.
    assert_eq!(decode_shell_output(b"\xEF\xF0\xE8\xEC\xE5\xF0"), "пример");
}

#[test]
fn test_cp866_shell_output() {
    // Native cmd.exe still defaults to CP866; make sure we recognize that too.
    assert_eq!(decode_shell_output(b"\xAF\xE0\xA8\xAC\xA5\xE0"), "пример");
}

#[test]
fn test_windows_1252_smart_decoding() {
    // Smart detection should turn fancy quotes/dashes into the proper Unicode glyphs.
    assert_eq!(
        decode_shell_output(b"\x93\x94 test \x96 dash"),
        "\u{201C}\u{201D} test \u{2013} dash"
    );
}

#[test]
fn test_smart_decoding_improves_over_lossy_utf8() {
    // Regression guard: String::from_utf8_lossy() alone used to emit replacement chars here.
    let bytes = b"\x93\x94 test \x96 dash";
    assert!(
        String::from_utf8_lossy(bytes).contains('\u{FFFD}'),
        "lossy UTF-8 should inject replacement chars"
    );
    assert_eq!(
        decode_shell_output(bytes),
        "\u{201C}\u{201D} test \u{2013} dash",
        "smart decoding should keep curly quotes intact"
    );
}

#[test]
fn test_mixed_ascii_and_legacy_encoding() {
    // Commands tend to mix ASCII status text with Latin-1 bytes (e.g. café).
    assert_eq!(decode_shell_output(b"Output: caf\xE9"), "Output: café"); // codespell:ignore caf
}

#[test]
fn test_pure_latin1_shell_output() {
    // Latin-1 by itself should still decode correctly (regression coverage for the older tests).
    assert_eq!(decode_shell_output(b"caf\xE9"), "café"); // codespell:ignore caf
}

#[test]
fn test_invalid_bytes_still_fall_back_to_lossy() {
    // If detection fails, we still want the user to see replacement characters.
    let bytes = b"\xFF\xFE\xFD";
    assert_eq!(decode_shell_output(bytes), String::from_utf8_lossy(bytes));
}

fn decode_shell_output(bytes: &[u8]) -> String {
    StreamOutput {
        text: bytes.to_vec(),
        truncated_after_lines: None,
    }
    .from_utf8_lossy()
    .text
}
