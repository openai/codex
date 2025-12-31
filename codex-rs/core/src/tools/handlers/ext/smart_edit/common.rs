//! Common utilities for smart edit operations
//!
//! This module provides text manipulation, file operations, and hashing
//! utilities used by both matching strategies and LLM correction.

use sha2::Digest;
use sha2::Sha256;

/// Compute SHA256 hash of content for concurrent modification detection
///
/// Used to detect if a file was modified externally between read and write operations.
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Detect line ending style (CRLF vs LF)
///
/// Simple heuristic: if file contains any \r\n, treat as CRLF.
/// Works for 99% of cases.
pub fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Unescape strings that may have been over-escaped by LLM generation
///
/// LLMs often over-escape strings, producing:
/// - `\\n` instead of `\n` (newline)
/// - `\\t` instead of `\t` (tab)
/// - `\\"` instead of `"` (quote)
/// - `\\'` instead of `'` (apostrophe)
/// - `\\`` instead of `` ` `` (backtick)
/// - `\\\\` instead of `\` (backslash)
///
/// This function detects and corrects these over-escape patterns.
/// Ported from gemini-cli's `unescapeStringForGeminiBug()`.
pub fn unescape_string_for_llm_bug(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Check if this is an escape sequence
            match chars.peek() {
                Some(&'n') => {
                    chars.next();
                    result.push('\n');
                }
                Some(&'t') => {
                    chars.next();
                    result.push('\t');
                }
                Some(&'r') => {
                    chars.next();
                    result.push('\r');
                }
                Some(&'\'') => {
                    chars.next();
                    result.push('\'');
                }
                Some(&'"') => {
                    chars.next();
                    result.push('"');
                }
                Some(&'`') => {
                    chars.next();
                    result.push('`');
                }
                Some(&'\\') => {
                    // Handle double backslash - consume the second one
                    chars.next();
                    // Check if there's an escape char after \\
                    match chars.peek() {
                        Some(&'n') => {
                            chars.next();
                            result.push('\n');
                        }
                        Some(&'t') => {
                            chars.next();
                            result.push('\t');
                        }
                        Some(&'r') => {
                            chars.next();
                            result.push('\r');
                        }
                        Some(&'\'') => {
                            chars.next();
                            result.push('\'');
                        }
                        Some(&'"') => {
                            chars.next();
                            result.push('"');
                        }
                        Some(&'`') => {
                            chars.next();
                            result.push('`');
                        }
                        Some(&'\\') => {
                            // More backslashes - just output one
                            result.push('\\');
                        }
                        _ => {
                            // Just \\ followed by something else - output single backslash
                            result.push('\\');
                        }
                    }
                }
                _ => {
                    // Single backslash not followed by escape char
                    result.push('\\');
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Count non-overlapping occurrences of a substring in a string
///
/// Returns 0 for empty needle (matches gemini-cli behavior).
/// Uses non-overlapping counting, same as Rust's `matches().count()`.
pub fn count_non_overlapping_occurrences(haystack: &str, needle: &str) -> i32 {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content_consistency() {
        let content = "test content";
        let hash1 = hash_content(content);
        let hash2 = hash_content(content);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_hash_content_different() {
        let hash1 = hash_content("content1");
        let hash2 = hash_content("content2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_detect_line_ending_lf() {
        assert_eq!(detect_line_ending("line1\nline2\nline3\n"), "\n");
        assert_eq!(detect_line_ending("single line"), "\n");
    }

    #[test]
    fn test_detect_line_ending_crlf() {
        assert_eq!(detect_line_ending("line1\r\nline2\r\n"), "\r\n");
        assert_eq!(detect_line_ending("mixed\r\nhas\ncrlf"), "\r\n"); // Any CRLF → CRLF
    }

    // Tests for unescape_string_for_llm_bug

    #[test]
    fn test_unescape_no_escapes() {
        assert_eq!(unescape_string_for_llm_bug("hello world"), "hello world");
        assert_eq!(unescape_string_for_llm_bug(""), "");
    }

    #[test]
    fn test_unescape_newline() {
        // \n should become actual newline
        assert_eq!(unescape_string_for_llm_bug("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_unescape_tab() {
        // \t should become actual tab
        assert_eq!(unescape_string_for_llm_bug("col1\\tcol2"), "col1\tcol2");
    }

    #[test]
    fn test_unescape_carriage_return() {
        // \r should become actual carriage return
        assert_eq!(unescape_string_for_llm_bug("line1\\rline2"), "line1\rline2");
    }

    #[test]
    fn test_unescape_quotes() {
        // \" should become "
        assert_eq!(
            unescape_string_for_llm_bug("say \\\"hello\\\""),
            "say \"hello\""
        );
        // \' should become '
        assert_eq!(
            unescape_string_for_llm_bug("it\\'s working"),
            "it's working"
        );
    }

    #[test]
    fn test_unescape_backtick() {
        // \` should become `
        assert_eq!(
            unescape_string_for_llm_bug("use \\`template\\`"),
            "use `template`"
        );
    }

    #[test]
    fn test_unescape_double_backslash() {
        // \\ should become single backslash when followed by escape char
        assert_eq!(unescape_string_for_llm_bug("path\\\\nname"), "path\nname");
        // \\\\ followed by n should become \n (backslash then newline)
        assert_eq!(unescape_string_for_llm_bug("path\\\\\\\\n"), "path\\\n");
    }

    #[test]
    fn test_unescape_mixed() {
        // Multiple escape sequences
        assert_eq!(
            unescape_string_for_llm_bug("line1\\nline2\\ttab\\\"quoted\\\""),
            "line1\nline2\ttab\"quoted\""
        );
    }

    #[test]
    fn test_unescape_trailing_backslash() {
        // Trailing backslash should be preserved
        assert_eq!(unescape_string_for_llm_bug("end\\"), "end\\");
    }

    #[test]
    fn test_unescape_backslash_not_escape() {
        // Backslash not followed by recognized escape char
        assert_eq!(unescape_string_for_llm_bug("\\a\\b\\c"), "\\a\\b\\c");
    }

    // Tests for count_non_overlapping_occurrences

    #[test]
    fn test_count_empty_needle() {
        assert_eq!(count_non_overlapping_occurrences("hello", ""), 0);
    }

    #[test]
    fn test_count_no_match() {
        assert_eq!(count_non_overlapping_occurrences("hello world", "xyz"), 0);
    }

    #[test]
    fn test_count_single_match() {
        assert_eq!(count_non_overlapping_occurrences("hello world", "world"), 1);
    }

    #[test]
    fn test_count_multiple_matches() {
        assert_eq!(
            count_non_overlapping_occurrences("foo bar foo baz foo", "foo"),
            3
        );
    }

    #[test]
    fn test_count_non_overlapping() {
        // "aa" in "aaaa" should be 2, not 3 (non-overlapping)
        assert_eq!(count_non_overlapping_occurrences("aaaa", "aa"), 2);
    }

    #[test]
    fn test_count_multiline() {
        let content = "line1\nline2\nline1\nline3";
        assert_eq!(count_non_overlapping_occurrences(content, "line1"), 2);
    }
}
