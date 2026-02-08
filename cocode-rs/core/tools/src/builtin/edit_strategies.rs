//! Three-tier matching strategies for the Edit tool.
//!
//! Progressive matching:
//! 1. **Exact** — fast literal string match
//! 2. **Flexible** — whitespace-insensitive with indentation preservation
//! 3. **Regex** — token-based fuzzy matching (first occurrence only)
//!
//! Plus pre-correction helpers for common LLM escaping bugs.

use regex_lite::NoExpand;
use regex_lite::Regex;

/// Which strategy was used to match `old_string`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    Exact,
    Flexible,
    Regex,
}

impl std::fmt::Display for MatchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact => write!(f, "exact"),
            Self::Flexible => write!(f, "flexible"),
            Self::Regex => write!(f, "regex"),
        }
    }
}

// ---------------------------------------------------------------------------
// Strategy 1: Exact
// ---------------------------------------------------------------------------

/// Try exact literal string replacement.
///
/// Returns `Some((replaced_content, occurrence_count))` on match.
pub fn try_exact_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Option<(String, usize)> {
    let count = content.matches(old_string).count();
    if count == 0 {
        return None;
    }
    let replaced = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    Some((replaced, count))
}

// ---------------------------------------------------------------------------
// Strategy 2: Flexible (whitespace-insensitive)
// ---------------------------------------------------------------------------

/// Try flexible matching: trim each line before comparison, preserve original indentation.
///
/// Returns `Some((replaced_content, occurrence_count))` on match.
pub fn try_flexible_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Option<(String, usize)> {
    let source_lines: Vec<&str> = content.lines().collect();
    let search_lines: Vec<&str> = old_string.lines().map(|l| l.trim()).collect();
    let replace_lines: Vec<&str> = new_string.lines().collect();

    if search_lines.is_empty() || search_lines.iter().all(|l| l.is_empty()) {
        return None;
    }

    let mut result_lines: Vec<String> = Vec::new();
    let mut i = 0;
    let mut occurrences = 0;

    while i < source_lines.len() {
        if i + search_lines.len() <= source_lines.len() {
            let window = &source_lines[i..i + search_lines.len()];
            let matches = window
                .iter()
                .zip(&search_lines)
                .all(|(src, search)| src.trim() == *search);

            if matches && (replace_all || occurrences == 0) {
                // Extract indentation from the first matched source line
                let first_src = window[0];
                let indent = &first_src[..first_src.len() - first_src.trim_start().len()];

                for (j, line) in replace_lines.iter().enumerate() {
                    if j == 0 {
                        result_lines.push(format!("{indent}{}", line.trim_start()));
                    } else if line.trim().is_empty() {
                        result_lines.push(String::new());
                    } else {
                        result_lines.push(format!("{indent}{line}"));
                    }
                }
                i += search_lines.len();
                occurrences += 1;
                if !replace_all {
                    // Copy remaining lines verbatim
                    result_lines.extend(source_lines[i..].iter().map(|s| s.to_string()));
                    break;
                }
                continue;
            }
        }
        result_lines.push(source_lines[i].to_string());
        i += 1;
    }

    if occurrences > 0 {
        let mut joined = result_lines.join("\n");
        if content.ends_with('\n') && !joined.ends_with('\n') {
            joined.push('\n');
        }
        Some((joined, occurrences))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Strategy 3: Regex (token-based fuzzy)
// ---------------------------------------------------------------------------

/// Try regex-based token matching (first occurrence only).
///
/// Tokenizes `old_string` by common delimiters, builds a pattern with flexible
/// whitespace, captures leading indentation and applies it to replacement.
pub fn try_regex_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
) -> Option<(String, usize)> {
    let delimiters = ['(', ')', ':', '[', ']', '{', '}', '>', '<', '='];
    let mut tokenized = old_string.to_string();
    for delim in delimiters {
        tokenized = tokenized.replace(delim, &format!(" {delim} "));
    }

    let tokens: Vec<&str> = tokenized.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let escaped_tokens: Vec<String> = tokens.iter().map(|t| escape_regex(t)).collect();
    let pattern = escaped_tokens.join(r"\s*");
    let final_pattern = format!(r"(?m)^(\s*){pattern}");

    let regex = Regex::new(&final_pattern).ok()?;
    let captures = regex.captures(content)?;
    let indentation = captures.get(1).map(|m| m.as_str()).unwrap_or("");

    let new_lines: Vec<String> = new_string
        .lines()
        .map(|line| format!("{indentation}{line}"))
        .collect();
    let new_block = new_lines.join("\n");

    let modified = regex
        .replace(content, NoExpand(new_block.as_str()))
        .to_string();

    // Preserve trailing newline
    let result = if content.ends_with('\n') && !modified.ends_with('\n') {
        format!("{modified}\n")
    } else if !content.ends_with('\n') && modified.ends_with('\n') {
        modified.trim_end_matches('\n').to_string()
    } else {
        modified
    };

    Some((result, 1))
}

/// Escape regex special characters.
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

// ---------------------------------------------------------------------------
// Pre-correction: unescape LLM over-escaping bugs
// ---------------------------------------------------------------------------

/// Unescape strings that may have been over-escaped by LLM generation.
///
/// LLMs often produce `\\n` instead of `\n`, `\\t` instead of `\t`, etc.
pub fn unescape_string_for_llm_bug(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
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
                            result.push('\\');
                        }
                        _ => {
                            result.push('\\');
                        }
                    }
                }
                _ => {
                    result.push('\\');
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Check if a string appears to be potentially over-escaped by an LLM.
fn is_potentially_over_escaped(s: &str) -> bool {
    s.contains("\\n")
        || s.contains("\\t")
        || s.contains("\\r")
        || s.contains("\\\"")
        || s.contains("\\'")
        || s.contains("\\`")
        || s.contains("\\\\")
}

/// Pre-correct escaping issues before trying matching strategies.
///
/// 1. If `old_string` has 0 exact matches but the unescaped version matches → use unescaped.
/// 2. If `old_string` matches but `new_string` appears over-escaped → unescape `new_string`.
pub fn pre_correct_escaping(old_string: &str, new_string: &str, content: &str) -> (String, String) {
    let occurrences = content.matches(old_string).count();

    // If old_string matches, check if new_string needs escaping correction
    if occurrences > 0 {
        if is_potentially_over_escaped(new_string) {
            let corrected_new = unescape_string_for_llm_bug(new_string);
            return (old_string.to_string(), corrected_new);
        }
        return (old_string.to_string(), new_string.to_string());
    }

    // If no match, try unescaping old_string
    let unescaped_old = unescape_string_for_llm_bug(old_string);
    let unescaped_occurrences = content.matches(&unescaped_old).count();

    if unescaped_occurrences > 0 {
        tracing::info!("Edit pre-correction: unescape fixed old_string match");
        let unescaped_new = unescape_string_for_llm_bug(new_string);
        return (unescaped_old, unescaped_new);
    }

    // No pre-correction helped
    (old_string.to_string(), new_string.to_string())
}

// ---------------------------------------------------------------------------
// Trim fallback
// ---------------------------------------------------------------------------

/// Try trimmed versions of old/new if original doesn't match.
///
/// If `old_string` has leading/trailing whitespace that prevents matching,
/// returns `Some((trimmed_old, trimmed_new))` when the trimmed version
/// is found in `content`.
pub fn trim_pair_if_possible(
    old_string: &str,
    new_string: &str,
    content: &str,
) -> Option<(String, String)> {
    let trimmed_old = old_string.trim();
    if trimmed_old.len() != old_string.len() && content.contains(trimmed_old) {
        Some((trimmed_old.to_string(), new_string.trim().to_string()))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the closest match in `content` to help diagnose why `old_string` wasn't found.
pub fn find_closest_match(content: &str, old_string: &str) -> String {
    let search_first_line = old_string.lines().next().unwrap_or("").trim();
    if search_first_line.is_empty() {
        return "old_string appears to be empty or whitespace-only".to_string();
    }

    let content_lines: Vec<&str> = content.lines().collect();
    let mut candidates = Vec::new();
    for (i, line) in content_lines.iter().enumerate() {
        if line.trim() == search_first_line {
            candidates.push(i);
        }
    }

    if candidates.is_empty() {
        format!(
            "The first line of old_string '{}' was not found anywhere in the file. \
             The file content may have changed since last read.",
            truncate_str(search_first_line, 80)
        )
    } else {
        let line_num = candidates[0] + 1;
        format!(
            "Found a partial match starting at line {line_num}. \
             The old_string may have incorrect indentation, extra/missing lines, or other differences. \
             Re-read the file to get the exact content."
        )
    }
}

/// Truncate a string to a maximum length, appending "..." if truncated.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..s.floor_char_boundary(max_len)])
    }
}

/// Compute simple diff statistics between original and modified content.
pub fn diff_stats(original: &str, modified: &str) -> String {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = modified.lines().collect();

    let mut changed = 0;
    let min_len = old_lines.len().min(new_lines.len());
    for i in 0..min_len {
        if old_lines[i] != new_lines[i] {
            changed += 1;
        }
    }

    let added = new_lines.len().saturating_sub(old_lines.len()) + changed;
    let removed = old_lines.len().saturating_sub(new_lines.len()) + changed;

    if added == 0 && removed == 0 {
        String::new()
    } else {
        format!(" (+{added}/-{removed} lines)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Exact ───────────────────────────────────────────────────────

    #[test]
    fn test_exact_replace_basic() {
        let (result, count) = try_exact_replace("hello world", "world", "rust", false).unwrap();
        assert_eq!(result, "hello rust");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_exact_replace_all() {
        let (result, count) = try_exact_replace("foo bar foo", "foo", "baz", true).unwrap();
        assert_eq!(result, "baz bar baz");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_exact_no_match() {
        assert!(try_exact_replace("hello", "xyz", "abc", false).is_none());
    }

    // ── Flexible ────────────────────────────────────────────────────

    #[test]
    fn test_flexible_replace_basic() {
        let content = "    let x = 1;\n    let y = 2;\n";
        let old = "let x = 1;\nlet y = 2;";
        let new = "let x = 10;\nlet y = 20;";
        let (result, count) = try_flexible_replace(content, old, new, false).unwrap();
        assert_eq!(count, 1);
        assert!(result.contains("    let x = 10;"));
        assert!(result.contains("    let y = 20;"));
    }

    #[test]
    fn test_flexible_replace_no_match() {
        assert!(try_flexible_replace("hello world\n", "nonexistent", "x", false).is_none());
    }

    #[test]
    fn test_flexible_replace_all_occurrences() {
        let content = "    foo bar\n    baz\n    foo bar\n    baz\n";
        let (result, count) =
            try_flexible_replace(content, "foo bar\nbaz", "replaced\nline", true).unwrap();
        assert_eq!(count, 2);
        assert_eq!(result.matches("replaced").count(), 2);
    }

    // ── Regex ───────────────────────────────────────────────────────

    #[test]
    fn test_regex_replace_intra_line_whitespace() {
        let content = "function test(){body}";
        let old = "function test ( ) { body }";
        let new = "function test(){updated}";
        let (result, count) = try_regex_replace(content, old, new).unwrap();
        assert_eq!(count, 1);
        assert!(result.contains("updated"));
    }

    #[test]
    fn test_regex_replace_first_only() {
        let content = "func(){}\nfunc(){}\n";
        let old = "func ( ) { }";
        let new = "updated(){}";
        let (result, count) = try_regex_replace(content, old, new).unwrap();
        assert_eq!(count, 1);
        // Only first occurrence replaced
        assert_eq!(result.matches("func(){}").count(), 1);
        assert!(result.contains("updated(){}"));
    }

    #[test]
    fn test_regex_replace_no_match() {
        assert!(try_regex_replace("hello world", "nonexistent_func()", "x").is_none());
    }

    // ── Pre-correction ──────────────────────────────────────────────

    #[test]
    fn test_pre_correct_no_change() {
        let (old, new) = pre_correct_escaping("hello", "hi", "hello world");
        assert_eq!(old, "hello");
        assert_eq!(new, "hi");
    }

    #[test]
    fn test_pre_correct_unescape_fixes_match() {
        let content = "line1\nline2";
        let (old, new) = pre_correct_escaping("line1\\nline2", "line1\\nupdated", content);
        assert_eq!(old, "line1\nline2");
        assert_eq!(new, "line1\nupdated");
    }

    #[test]
    fn test_pre_correct_new_string_over_escaped() {
        let content = "hello world";
        let (old, new) = pre_correct_escaping("hello", "hi\\nthere", content);
        assert_eq!(old, "hello");
        assert_eq!(new, "hi\nthere");
    }

    #[test]
    fn test_pre_correct_no_help() {
        let (old, new) = pre_correct_escaping("notfound", "replacement", "hello world");
        assert_eq!(old, "notfound");
        assert_eq!(new, "replacement");
    }

    // ── Unescape ────────────────────────────────────────────────────

    #[test]
    fn test_unescape_no_escapes() {
        assert_eq!(unescape_string_for_llm_bug("hello world"), "hello world");
    }

    #[test]
    fn test_unescape_newline() {
        assert_eq!(unescape_string_for_llm_bug("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_unescape_tab() {
        assert_eq!(unescape_string_for_llm_bug("col1\\tcol2"), "col1\tcol2");
    }

    #[test]
    fn test_unescape_quotes() {
        assert_eq!(
            unescape_string_for_llm_bug("say \\\"hello\\\""),
            "say \"hello\""
        );
        assert_eq!(
            unescape_string_for_llm_bug("it\\'s working"),
            "it's working"
        );
    }

    #[test]
    fn test_unescape_double_backslash() {
        assert_eq!(unescape_string_for_llm_bug("path\\\\nname"), "path\nname");
    }

    #[test]
    fn test_unescape_trailing_backslash() {
        assert_eq!(unescape_string_for_llm_bug("end\\"), "end\\");
    }

    #[test]
    fn test_unescape_backslash_not_escape() {
        assert_eq!(unescape_string_for_llm_bug("\\a\\b\\c"), "\\a\\b\\c");
    }

    // ── Trim pair ───────────────────────────────────────────────────

    #[test]
    fn test_trim_pair_no_trimming_needed() {
        assert!(trim_pair_if_possible("hello", "world", "hello there").is_none());
    }

    #[test]
    fn test_trim_pair_trimming_helps() {
        let (old, new) = trim_pair_if_possible("  hello  ", "  hi  ", "hello world").unwrap();
        assert_eq!(old, "hello");
        assert_eq!(new, "hi");
    }

    #[test]
    fn test_trim_pair_no_content_match() {
        assert!(trim_pair_if_possible("  xyz  ", "  abc  ", "hello world").is_none());
    }

    // ── Helpers ─────────────────────────────────────────────────────

    #[test]
    fn test_find_closest_match_found() {
        let hint = find_closest_match(
            "fn main() {\n    let x = 1;\n}\n",
            "fn main() {\n    let x = 2;\n}",
        );
        assert!(hint.contains("partial match"));
    }

    #[test]
    fn test_find_closest_match_not_found() {
        let hint = find_closest_match("fn main() {}\n", "nonexistent_function()");
        assert!(hint.contains("not found anywhere"));
    }

    #[test]
    fn test_diff_stats() {
        assert_eq!(diff_stats("a\nb\nc\n", "a\nB\nc\n"), " (+1/-1 lines)");
        assert_eq!(diff_stats("a\n", "a\nb\n"), " (+1/-0 lines)");
        assert_eq!(diff_stats("a\nb\n", "a\n"), " (+0/-1 lines)");
        assert_eq!(diff_stats("same\n", "same\n"), "");
    }

    // ── Regex: NoExpand ($-in-replacement) ─────────────────────────

    #[test]
    fn test_regex_replace_dollar_in_replacement() {
        // $0 should NOT be expanded as a capture group reference
        let content = "function test(){body}";
        let old = "function test ( ) { body }";
        let new = "function cost(){ $0 }";
        let (result, _) = try_regex_replace(content, old, new).unwrap();
        assert!(
            result.contains("$0"),
            "Literal $0 should be preserved, got: {result}"
        );

        // $HOME should NOT be expanded
        let new2 = "echo $HOME";
        let (result2, _) = try_regex_replace(content, old, new2).unwrap();
        assert!(
            result2.contains("$HOME"),
            "Literal $HOME should be preserved, got: {result2}"
        );

        // $1 should NOT be expanded
        let new3 = "let cost = $1.00";
        let (result3, _) = try_regex_replace(content, old, new3).unwrap();
        assert!(
            result3.contains("$1.00"),
            "Literal $1.00 should be preserved, got: {result3}"
        );
    }

    // ── escape_regex special chars ─────────────────────────────────

    #[test]
    fn test_escape_regex_special_chars() {
        assert_eq!(escape_regex(r"\"), r"\\");
        assert_eq!(escape_regex("."), r"\.");
        assert_eq!(escape_regex("$"), r"\$");
        assert_eq!(escape_regex("|"), r"\|");
        assert_eq!(escape_regex("("), r"\(");
        assert_eq!(escape_regex(")"), r"\)");
        assert_eq!(escape_regex("["), r"\[");
        assert_eq!(escape_regex("]"), r"\]");
        assert_eq!(escape_regex("{"), r"\{");
        assert_eq!(escape_regex("}"), r"\}");
        assert_eq!(escape_regex("^"), r"\^");
        assert_eq!(escape_regex("+"), r"\+");
        assert_eq!(escape_regex("*"), r"\*");
        assert_eq!(escape_regex("?"), r"\?");
        // Non-special chars pass through
        assert_eq!(escape_regex("abc"), "abc");
        // Mixed
        assert_eq!(escape_regex("a.b"), r"a\.b");
        assert_eq!(escape_regex("$HOME"), r"\$HOME");
    }

    // ── Regex trailing newline preservation ─────────────────────────

    #[test]
    fn test_regex_trailing_newline_both_have() {
        // Content ends with \n, replacement preserves it
        let content = "  func(){body}\n";
        let old = "func ( ) { body }";
        let new = "func(){updated}";
        let (result, _) = try_regex_replace(content, old, new).unwrap();
        assert!(
            result.ends_with('\n'),
            "Should preserve trailing newline, got: {result:?}"
        );
    }

    #[test]
    fn test_regex_trailing_newline_neither_has() {
        // Content does NOT end with \n
        let content = "  func(){body}";
        let old = "func ( ) { body }";
        let new = "func(){updated}";
        let (result, _) = try_regex_replace(content, old, new).unwrap();
        assert!(
            !result.ends_with('\n'),
            "Should NOT add trailing newline, got: {result:?}"
        );
    }

    #[test]
    fn test_regex_trailing_newline_content_has_replacement_adds() {
        // Content ends with \n, regex replace might add extra — should stay single \n
        let content = "  func(){body}\n";
        let old = "func ( ) { body }";
        let new = "func(){updated}\n";
        let (result, _) = try_regex_replace(content, old, new).unwrap();
        assert!(
            result.ends_with('\n'),
            "Should have trailing newline, got: {result:?}"
        );
    }

    #[test]
    fn test_regex_trailing_newline_content_lacks_replacement_adds() {
        // Content does NOT end with \n, but replacement does — should strip
        let content = "  func(){body}";
        let old = "func ( ) { body }";
        let new = "func(){updated}\n";
        let (result, _) = try_regex_replace(content, old, new).unwrap();
        assert!(
            !result.ends_with('\n'),
            "Should strip trailing newline to match original, got: {result:?}"
        );
    }
}
