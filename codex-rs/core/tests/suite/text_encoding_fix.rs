//! Integration test for the text encoding fix for issue #6178.
//!
//! This test simulates the scenario where VSCode shell preview window
//! shows garbled text when executing commands with non-ASCII characters
//! in Windows/WSL environments.

use codex_core::exec::StreamOutput;

/// Test that simulates the specific issue #6178 scenario:
/// - User runs `codex exec "пример"` (Russian text)
/// - In Windows/WSL environment, shell output might be encoded in Windows-1252
/// - Our fix should correctly decode this without showing replacement characters
#[test]
fn test_shell_output_encoding_issue_6178() {
    // Simulate various encoding scenarios that could happen in Windows/WSL

    // Test case 1: UTF-8 Russian text (should work fine)
    let utf8_russian = "пример".as_bytes();
    let utf8_output = StreamOutput {
        text: utf8_russian.to_vec(),
        truncated_after_lines: None,
    };
    let decoded_utf8 = utf8_output.from_utf8_lossy();
    assert_eq!(decoded_utf8.text, "пример");
    assert!(!decoded_utf8.text.contains('\u{FFFD}')); // No replacement characters

    // Test case 2: Windows-1252 encoded text (simulating Windows shell output)
    // This represents bytes that might come from a Windows process
    let windows_bytes = [0x93, 0x94, 0x20, 0x74, 0x65, 0x73, 0x74]; // ""test" in Windows-1252
    let windows_output = StreamOutput {
        text: windows_bytes.to_vec(),
        truncated_after_lines: None,
    };
    let decoded_windows = windows_output.from_utf8_lossy();
    assert!(!decoded_windows.text.contains('\u{FFFD}')); // Should not have replacement chars
    assert!(decoded_windows.text.contains("test")); // Should contain the ASCII part

    // Test case 3: Latin-1 encoded text (common fallback)
    let latin1_text = "café"; // This would be [99, 97, 102, 233] in Latin-1
    let latin1_bytes = latin1_text.encode_latin1();
    let latin1_output = StreamOutput {
        text: latin1_bytes,
        truncated_after_lines: None,
    };
    let decoded_latin1 = latin1_output.from_utf8_lossy();
    assert_eq!(decoded_latin1.text, "café");
    assert!(!decoded_latin1.text.contains('\u{FFFD}'));

    // Test case 4: Mixed content (common in real scenarios)
    let mut mixed_bytes = Vec::new();
    mixed_bytes.extend_from_slice("Output: ".as_bytes()); // ASCII prefix
    mixed_bytes.extend_from_slice(&latin1_text.encode_latin1()); // Latin-1 content
    let mixed_output = StreamOutput {
        text: mixed_bytes,
        truncated_after_lines: None,
    };
    let decoded_mixed = mixed_output.from_utf8_lossy();
    assert!(decoded_mixed.text.starts_with("Output: "));
    assert!(decoded_mixed.text.contains("café"));
    assert!(!decoded_mixed.text.contains('\u{FFFD}'));
}

/// Test that demonstrates the improvement over the old approach
#[test]
fn test_improvement_over_string_from_utf8_lossy() {
    // This test shows that our smart decoding handles cases where
    // String::from_utf8_lossy() would produce replacement characters

    // Windows-1252 bytes that would be invalid UTF-8
    let problematic_bytes = [0x93, 0x94]; // LEFT and RIGHT DOUBLE QUOTATION MARK

    // Old approach (what was happening before our fix)
    let old_result = String::from_utf8_lossy(&problematic_bytes).to_string();
    assert!(old_result.contains('\u{FFFD}')); // Contains replacement characters

    // New approach (our fix)
    let new_output = StreamOutput {
        text: problematic_bytes.to_vec(),
        truncated_after_lines: None,
    };
    let new_result = new_output.from_utf8_lossy();
    assert!(!new_result.text.contains('\u{FFFD}')); // No replacement characters
    assert_eq!(new_result.text, "\u{201C}\u{201D}"); // Correct Unicode characters
}

/// Helper trait to simulate Latin-1 encoding
trait EncodeLatin1 {
    fn encode_latin1(&self) -> Vec<u8>;
}

impl EncodeLatin1 for str {
    fn encode_latin1(&self) -> Vec<u8> {
        self.chars()
            .map(|c| {
                let code = c as u32;
                if code <= 255 {
                    code as u8
                } else {
                    b'?' // Replacement for non-Latin-1 characters
                }
            })
            .collect()
    }
}
