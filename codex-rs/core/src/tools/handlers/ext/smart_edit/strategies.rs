//! Three-tier matching strategies for smart edit
//!
//! This module implements progressive matching strategies:
//! 1. Exact: Fast literal string match
//! 2. Flexible: Whitespace-insensitive with indentation preservation
//! 3. Regex: Token-based fuzzy matching (first occurrence only)

use regex_lite::Regex;

/// Result of a replacement attempt
#[derive(Debug, Clone)]
pub struct ReplacementResult {
    pub new_content: String,
    pub occurrences: i32,
    pub strategy: String,
}

/// Restore trailing newline behavior to match original content
///
/// If original had a trailing newline, ensure modified has one.
/// If original didn't have a trailing newline, remove it from modified.
/// This matches gemini-cli's restoreTrailingNewline() behavior.
fn restore_trailing_newline(original: &str, modified: &str) -> String {
    let had_trailing = original.ends_with('\n');
    if had_trailing && !modified.ends_with('\n') {
        format!("{modified}\n")
    } else if !had_trailing && modified.ends_with('\n') {
        modified.trim_end_matches('\n').to_string()
    } else {
        modified.to_string()
    }
}

/// Try all strategies in order until one succeeds
///
/// Returns immediately on first successful match, or returns failure result
/// with 0 occurrences if all strategies fail.
pub fn try_all_strategies(old: &str, new: &str, content: &str) -> ReplacementResult {
    // Strategy 1: Exact match (fastest)
    if let Some((new_content, count)) = try_exact_replacement(old, new, content) {
        let result = ReplacementResult {
            new_content,
            occurrences: count,
            strategy: "exact".to_string(),
        };
        tracing::info!(
            strategy = "exact",
            occurrences = count,
            "Smart edit strategy succeeded"
        );
        return result;
    }

    // Strategy 2: Flexible match (whitespace-insensitive)
    if let Some((new_content, count)) = try_flexible_replacement(old, new, content) {
        let result = ReplacementResult {
            new_content,
            occurrences: count,
            strategy: "flexible".to_string(),
        };
        tracing::info!(
            strategy = "flexible",
            occurrences = count,
            "Smart edit strategy succeeded"
        );
        return result;
    }

    // Strategy 3: Regex match (token-based, first occurrence only)
    if let Some((new_content, count)) = try_regex_replacement(old, new, content) {
        let result = ReplacementResult {
            new_content,
            occurrences: count,
            strategy: "regex".to_string(),
        };
        tracing::info!(
            strategy = "regex",
            occurrences = count,
            "Smart edit strategy succeeded"
        );
        return result;
    }

    // All strategies failed
    tracing::warn!("Smart edit: all strategies failed, no match found");
    ReplacementResult {
        new_content: content.to_string(),
        occurrences: 0,
        strategy: "none".to_string(),
    }
}

/// Strategy 1: Exact literal string matching
///
/// Fast path - uses Rust's built-in string replace which is already literal.
fn try_exact_replacement(old: &str, new: &str, content: &str) -> Option<(String, i32)> {
    let occurrences = content.matches(old).count() as i32;
    if occurrences > 0 {
        let modified = content.replace(old, new);
        let new_content = restore_trailing_newline(content, &modified);
        Some((new_content, occurrences))
    } else {
        None
    }
}

/// Strategy 2: Flexible matching (whitespace-insensitive with indentation preservation)
///
/// This strategy:
/// - Strips leading/trailing whitespace from each line for comparison
/// - Preserves the original indentation of the matched block
/// - Applies that indentation to the replacement text
///
/// Example:
/// ```ignore
/// File has:     "    fn hello() {\n        code();\n    }"
/// Search for:   "fn hello() {\n    code();\n}"  (no leading spaces)
/// Result:       Matches! Replacement preserves 4-space indent
/// ```
fn try_flexible_replacement(old: &str, new: &str, content: &str) -> Option<(String, i32)> {
    let source_lines: Vec<&str> = content.lines().collect();
    let search_lines_stripped: Vec<String> =
        old.lines().map(|line| line.trim().to_string()).collect();
    let replace_lines: Vec<&str> = new.lines().collect();

    if search_lines_stripped.is_empty() {
        return None;
    }

    let mut result_lines = Vec::new();
    let mut occurrences = 0;
    let mut i = 0;

    // Sliding window to find matches
    while i
        <= source_lines
            .len()
            .saturating_sub(search_lines_stripped.len())
    {
        let window = &source_lines[i..i + search_lines_stripped.len()];
        let window_stripped: Vec<String> =
            window.iter().map(|line| line.trim().to_string()).collect();

        // Compare stripped versions
        let is_match = window_stripped
            .iter()
            .zip(&search_lines_stripped)
            .all(|(w, s)| w == s);

        if is_match {
            occurrences += 1;

            // Extract indentation from first line of match
            let indentation = extract_indentation(window[0]);

            // Apply replacement with preserved indentation
            for line in &replace_lines {
                result_lines.push(format!("{indentation}{line}"));
            }

            i += search_lines_stripped.len();
        } else {
            result_lines.push(source_lines[i].to_string());
            i += 1;
        }
    }

    // Add remaining lines
    while i < source_lines.len() {
        result_lines.push(source_lines[i].to_string());
        i += 1;
    }

    if occurrences > 0 {
        let modified = result_lines.join("\n");
        let new_content = restore_trailing_newline(content, &modified);
        Some((new_content, occurrences))
    } else {
        None
    }
}

/// Strategy 3: Regex-based token matching (first occurrence only)
///
/// This strategy:
/// - Tokenizes the old_string by common delimiters
/// - Creates a regex pattern with flexible whitespace between tokens
/// - Captures leading indentation
/// - Replaces ONLY the first occurrence (conservative approach)
///
/// Example:
/// ```ignore
/// Search: "function test ( ) {"
/// Pattern: "function\s*test\s*\(\s*\)\s*\{"
/// Matches: "function test(){" or "function  test  (  )  {"
/// ```
fn try_regex_replacement(old: &str, new: &str, content: &str) -> Option<(String, i32)> {
    // Tokenize by delimiters
    let delimiters = ['(', ')', ':', '[', ']', '{', '}', '>', '<', '='];
    let mut tokenized = old.to_string();
    for delim in delimiters {
        tokenized = tokenized.replace(delim, &format!(" {delim} "));
    }

    // Extract tokens
    let tokens: Vec<&str> = tokenized.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    // Escape regex special characters in each token
    let escaped_tokens: Vec<String> = tokens.iter().map(|t| escape_regex(t)).collect();

    // Join with flexible whitespace pattern
    let pattern = escaped_tokens.join(r"\s*");

    // Capture leading indentation
    let final_pattern = format!(r"^(\s*){pattern}");

    // Compile regex (multiline mode)
    let regex = match Regex::new(&final_pattern) {
        Ok(r) => r,
        Err(_) => return None,
    };

    // Find first match
    let captures = regex.captures(content)?;
    let indentation = captures.get(1).map(|m| m.as_str()).unwrap_or("");

    // Apply replacement with preserved indentation
    let new_lines: Vec<String> = new
        .lines()
        .map(|line| format!("{indentation}{line}"))
        .collect();
    let new_block = new_lines.join("\n");

    // Replace only first occurrence
    let modified = regex.replace(content, new_block.as_str()).to_string();
    let new_content = restore_trailing_newline(content, &modified);

    Some((new_content, 1)) // Always 1 occurrence for regex strategy
}

/// Extract leading indentation (spaces/tabs) from a line
fn extract_indentation(line: &str) -> &str {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    &line[..indent_len]
}

/// Try trimmed versions of old_string and new_string if original doesn't match
///
/// This is a fallback strategy when the original old_string has leading/trailing
/// whitespace that prevents matching. If the trimmed version matches the expected
/// occurrence count, use trimmed versions for both old and new strings.
///
/// Ported from gemini-cli's `trimPairIfPossible()`.
///
/// # Arguments
/// * `old` - Original search string
/// * `new` - Original replacement string
/// * `content` - File content to search in
/// * `expected` - Expected number of occurrences
///
/// # Returns
/// * `Some((trimmed_old, trimmed_new))` if trimmed version matches expected count
/// * `None` if trimming doesn't help or isn't applicable
pub fn trim_pair_if_possible(
    old: &str,
    new: &str,
    content: &str,
    expected: i32,
) -> Option<(String, String)> {
    let trimmed_old = old.trim();

    // Only try if trimming actually changes the string
    if trimmed_old.len() != old.len() {
        let trimmed_occurrences = content.matches(trimmed_old).count() as i32;
        if trimmed_occurrences == expected {
            return Some((trimmed_old.to_string(), new.trim().to_string()));
        }
    }

    None
}

/// Escape regex special characters
fn escape_regex(s: &str) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_replacement_basic() {
        let content = "hello world";
        let result = try_exact_replacement("world", "rust", content);
        assert!(result.is_some());
        let (new_content, count) = result.unwrap();
        assert_eq!(new_content, "hello rust");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_exact_replacement_multiple() {
        let content = "foo bar foo baz foo";
        let result = try_exact_replacement("foo", "qux", content);
        assert!(result.is_some());
        let (new_content, count) = result.unwrap();
        assert_eq!(new_content, "qux bar qux baz qux");
        assert_eq!(count, 3);
    }

    #[test]
    fn test_exact_replacement_not_found() {
        let content = "hello world";
        let result = try_exact_replacement("notfound", "replacement", content);
        assert!(result.is_none());
    }

    #[test]
    fn test_flexible_replacement_indentation() {
        let content = "fn test() {\n    old_code();\n}";
        let old = "old_code();"; // No indentation in search
        let new = "new_code();";

        let result = try_flexible_replacement(old, new, content);
        assert!(result.is_some());

        let (new_content, count) = result.unwrap();
        assert_eq!(count, 1);
        assert!(new_content.contains("    new_code();")); // 4-space indent preserved
    }

    #[test]
    fn test_flexible_replacement_multiline() {
        let content = "    if true {\n        code1();\n        code2();\n    }";
        let old = "if true {\n    code1();\n    code2();\n}"; // Different indent
        let new = "if false {\n    updated();\n}";

        let result = try_flexible_replacement(old, new, content);
        assert!(result.is_some());

        let (new_content, count) = result.unwrap();
        assert_eq!(count, 1);
        assert!(new_content.contains("    if false {")); // Original 4-space indent
    }

    #[test]
    fn test_flexible_replacement_not_found() {
        let content = "existing code";
        let result = try_flexible_replacement("notfound", "replacement", content);
        assert!(result.is_none());
    }

    #[test]
    fn test_regex_replacement_basic() {
        let content = "function test(){body}";
        let old = "function test ( ) { body }"; // With spaces
        let new = "function test(){updated}";

        let result = try_regex_replacement(old, new, content);
        assert!(result.is_some());

        let (new_content, count) = result.unwrap();
        assert_eq!(count, 1);
        assert!(new_content.contains("function test(){updated}"));
    }

    #[test]
    fn test_regex_replacement_first_only() {
        let content = "func(){}\nfunc(){}\nfunc(){}";
        let old = "func ( ) { }";
        let new = "updated(){}";

        let result = try_regex_replacement(old, new, content);
        assert!(result.is_some());

        let (new_content, count) = result.unwrap();
        assert_eq!(count, 1); // Only first occurrence
        assert!(new_content.starts_with("updated(){}"));
        assert!(new_content.contains("\nfunc(){}\nfunc(){}")); // Others unchanged
    }

    #[test]
    fn test_try_all_strategies_exact() {
        let result = try_all_strategies("exact", "match", "this is exact match");
        assert_eq!(result.strategy, "exact");
        assert_eq!(result.occurrences, 1);
        assert!(result.new_content.contains("this is match match"));
    }

    #[test]
    fn test_try_all_strategies_flexible() {
        // Multi-line: exact fails (search has no leading spaces), flexible works
        let content = "    line1\n    line2";
        let result = try_all_strategies("line1\nline2", "updated", content);
        assert_eq!(result.strategy, "flexible");
        assert_eq!(result.occurrences, 1);
    }

    #[test]
    fn test_try_all_strategies_none() {
        let result = try_all_strategies("notfound", "replacement", "some content");
        assert_eq!(result.strategy, "none");
        assert_eq!(result.occurrences, 0);
    }

    #[test]
    fn test_extract_indentation() {
        assert_eq!(extract_indentation("    code"), "    ");
        assert_eq!(extract_indentation("\t\tcode"), "\t\t");
        assert_eq!(extract_indentation("noindent"), "");
        assert_eq!(extract_indentation("  \t  mixed"), "  \t  ");
    }

    #[test]
    fn test_escape_regex() {
        assert_eq!(escape_regex("hello"), "hello");
        assert_eq!(escape_regex("a.b"), "a\\.b");
        assert_eq!(escape_regex("(test)"), "\\(test\\)");
        // Hyphen not escaped - only special inside char class, but we escape brackets
        assert_eq!(escape_regex("[a-z]+"), "\\[a-z\\]\\+");
    }

    #[test]
    fn test_restore_trailing_newline_preserve() {
        // Original has trailing newline, modified doesn't -> add it
        let result = restore_trailing_newline("content\n", "modified");
        assert_eq!(result, "modified\n");
    }

    #[test]
    fn test_restore_trailing_newline_remove() {
        // Original has no trailing newline, modified does -> remove it
        let result = restore_trailing_newline("content", "modified\n");
        assert_eq!(result, "modified");
    }

    #[test]
    fn test_restore_trailing_newline_both_have() {
        // Both have trailing newline -> keep as is
        let result = restore_trailing_newline("content\n", "modified\n");
        assert_eq!(result, "modified\n");
    }

    #[test]
    fn test_restore_trailing_newline_neither_have() {
        // Neither has trailing newline -> keep as is
        let result = restore_trailing_newline("content", "modified");
        assert_eq!(result, "modified");
    }

    // Tests for trim_pair_if_possible

    #[test]
    fn test_trim_pair_no_trimming_needed() {
        // No leading/trailing whitespace - returns None
        let result = trim_pair_if_possible("hello", "world", "hello there", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_trim_pair_trimming_helps() {
        // Leading whitespace prevents match, trimming fixes it
        let content = "hello world";
        let result = trim_pair_if_possible("  hello  ", "  hi  ", content, 1);
        assert!(result.is_some());
        let (trimmed_old, trimmed_new) = result.unwrap();
        assert_eq!(trimmed_old, "hello");
        assert_eq!(trimmed_new, "hi");
    }

    #[test]
    fn test_trim_pair_count_mismatch() {
        // Trimmed version has wrong occurrence count
        let content = "foo bar foo baz";
        // Trimmed "foo" has 2 occurrences, but expected is 1
        let result = trim_pair_if_possible("  foo  ", "  qux  ", content, 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_trim_pair_multiple_occurrences() {
        // Trimmed version matches expected multiple occurrences
        let content = "foo bar foo baz foo";
        let result = trim_pair_if_possible(" foo ", " qux ", content, 3);
        assert!(result.is_some());
        let (trimmed_old, trimmed_new) = result.unwrap();
        assert_eq!(trimmed_old, "foo");
        assert_eq!(trimmed_new, "qux");
    }
}
