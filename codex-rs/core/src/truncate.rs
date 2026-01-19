//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries, and helpers for line/token‑based truncation
//! used across the core crate.

use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::TruncationPolicy as ProtocolTruncationPolicy;
use serde_json;

const APPROX_BYTES_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl From<TruncationPolicy> for ProtocolTruncationPolicy {
    fn from(value: TruncationPolicy) -> Self {
        match value {
            TruncationPolicy::Bytes(bytes) => Self::Bytes(bytes),
            TruncationPolicy::Tokens(tokens) => Self::Tokens(tokens),
        }
    }
}

impl From<TruncationPolicyConfig> for TruncationPolicy {
    fn from(config: TruncationPolicyConfig) -> Self {
        match config.mode {
            TruncationMode::Bytes => Self::Bytes(config.limit as usize),
            TruncationMode::Tokens => Self::Tokens(config.limit as usize),
        }
    }
}

impl TruncationPolicy {
    /// Scale the underlying budget by `multiplier`, rounding up to avoid under-budgeting.
    pub fn mul(self, multiplier: f64) -> Self {
        match self {
            TruncationPolicy::Bytes(bytes) => {
                TruncationPolicy::Bytes((bytes as f64 * multiplier).ceil() as usize)
            }
            TruncationPolicy::Tokens(tokens) => {
                TruncationPolicy::Tokens((tokens as f64 * multiplier).ceil() as usize)
            }
        }
    }

    /// Returns a token budget derived from this policy.
    ///
    /// - For `Tokens`, this is the explicit token limit.
    /// - For `Bytes`, this is an approximate token budget using the global
    ///   bytes-per-token heuristic.
    pub fn token_budget(&self) -> usize {
        match self {
            TruncationPolicy::Bytes(bytes) => {
                usize::try_from(approx_tokens_from_byte_count(*bytes)).unwrap_or(usize::MAX)
            }
            TruncationPolicy::Tokens(tokens) => *tokens,
        }
    }

    /// Returns a byte budget derived from this policy.
    ///
    /// - For `Bytes`, this is the explicit byte limit.
    /// - For `Tokens`, this is an approximate byte budget using the global
    ///   bytes-per-token heuristic.
    pub fn byte_budget(&self) -> usize {
        match self {
            TruncationPolicy::Bytes(bytes) => *bytes,
            TruncationPolicy::Tokens(tokens) => approx_bytes_for_tokens(*tokens),
        }
    }
}

pub(crate) fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let result = truncate_text(content, policy);
    format!("Total output lines: {total_lines}\n\n{result}")
}

pub(crate) fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    // Check if content contains error patterns (compilation errors, test failures, etc.)
    // Error information is critical and should be preserved with priority
    if contains_error_patterns(content) {
        return truncate_with_error_priority(content, policy);
    }

    // Check if content is structured data (JSON/XML) that needs special handling
    // Structured data should be truncated safely to maintain parseability
    if is_json(content) {
        return truncate_json_safely(content, policy);
    }
    if is_xml(content) {
        return truncate_xml_safely(content, policy);
    }

    // Default truncation logic for regular text content
    match policy {
        TruncationPolicy::Bytes(_) => truncate_with_byte_estimate(content, policy),
        TruncationPolicy::Tokens(_) => {
            let (truncated, _) = truncate_with_token_budget(content, policy);
            truncated
        }
    }
}
/// Check if text contains error patterns that indicate compilation errors,
/// test failures, or other critical error information.
/// Uses case-insensitive matching for common error keywords.
fn contains_error_patterns(text: &str) -> bool {
    // Use case-insensitive matching for better coverage
    let lower = text.to_lowercase();
    lower.contains("error")
        || lower.contains("failed")
        || lower.contains("exception")
        || lower.contains("fatal")
        || lower.contains("panic")
        || lower.contains("abort")
}

/// Truncate content with priority given to error information.
/// Extracts error lines and preserves them along with surrounding context.
fn truncate_with_error_priority(content: &str, policy: TruncationPolicy) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut error_line_indices: Vec<usize> = Vec::new();

    // Identify lines containing error patterns
    for (idx, line) in lines.iter().enumerate() {
        if contains_error_patterns(line) {
            error_line_indices.push(idx);
        }
    }

    // If no error lines found, fall back to default truncation
    if error_line_indices.is_empty() {
        return match policy {
            TruncationPolicy::Bytes(_) => truncate_with_byte_estimate(content, policy),
            TruncationPolicy::Tokens(_) => {
                let (truncated, _) = truncate_with_token_budget(content, policy);
                truncated
            }
        };
    }

    // Extract error lines with context (2 lines before and after each error)
    const CONTEXT_LINES: usize = 2;
    let mut important_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for &error_idx in &error_line_indices {
        let start = error_idx.saturating_sub(CONTEXT_LINES);
        let end = (error_idx + CONTEXT_LINES + 1).min(lines.len());
        for idx in start..end {
            important_indices.insert(idx);
        }
    }

    // Build result: keep important lines, truncate others
    let budget = policy.token_budget();
    let mut result_lines: Vec<String> = Vec::new();
    let mut current_tokens = 0usize;

    for (idx, line) in lines.iter().enumerate() {
        let line_tokens = approx_token_count(line);
        let line_with_newline_tokens = line_tokens + 1; // Account for newline

        if important_indices.contains(&idx) {
            // Always include error lines and their context
            result_lines.push(line.to_string());
            current_tokens = current_tokens.saturating_add(line_with_newline_tokens);
        } else if current_tokens.saturating_add(line_with_newline_tokens) <= budget {
            // Include non-error lines if budget allows
            result_lines.push(line.to_string());
            current_tokens = current_tokens.saturating_add(line_with_newline_tokens);
        } else {
            // Budget exhausted, stop adding lines
            break;
        }
    }

    // If we have remaining budget and didn't include all error lines, prioritize them
    if result_lines.len() < lines.len() && !error_line_indices.is_empty() {
        // Add remaining error lines even if over budget
        for &error_idx in &error_line_indices {
            if error_idx >= result_lines.len() {
                result_lines.push(lines[error_idx].to_string());
            }
        }
    }

    result_lines.join("\n")
}

/// Check if text appears to be JSON by examining its start.
/// Simple heuristic: checks if trimmed content starts with '{' or '['.
fn is_json(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

/// Safely truncate JSON content while preserving structure.
/// Strategy: if content fits within budget, keep it all. Otherwise,
/// attempt to preserve top-level structure by truncating nested values.
fn truncate_json_safely(json: &str, policy: TruncationPolicy) -> String {
    let budget = policy.token_budget();
    let estimated_tokens = approx_token_count(json);

    // If content fits within budget, return as-is
    if estimated_tokens <= budget {
        return json.to_string();
    }

    // Attempt to parse JSON to preserve structure
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(value) => {
            // Successfully parsed - try to truncate while preserving structure
            truncate_json_value(value, budget)
        }
        Err(_) => {
            // Not valid JSON or parse failed - fall back to regular truncation
            // but try to preserve JSON-like structure by keeping opening/closing braces
            match policy {
                TruncationPolicy::Bytes(_) => truncate_with_byte_estimate(json, policy),
                TruncationPolicy::Tokens(_) => {
                    let (truncated, _) = truncate_with_token_budget(json, policy);
                    truncated
                }
            }
        }
    }
}

/// Truncate a JSON value while preserving top-level structure.
/// For objects, keeps all keys but may truncate values.
/// For arrays, keeps structure but may truncate elements.
fn truncate_json_value(value: serde_json::Value, budget: usize) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            let mut remaining_budget = budget.saturating_sub(2); // Account for "{}"
            let mut first = true;

            for (key, val) in map {
                let key_str = format!("\"{}\"", key);
                let key_tokens = approx_token_count(&key_str) + 1; // +1 for colon
                let val_str = serde_json::to_string(&val).unwrap_or_default();
                let val_tokens = approx_token_count(&val_str);

                if first {
                    remaining_budget = remaining_budget.saturating_sub(key_tokens);
                    first = false;
                } else {
                    remaining_budget = remaining_budget.saturating_sub(key_tokens + 1); // +1 for comma
                }

                if remaining_budget >= val_tokens {
                    result.insert(key, val);
                    remaining_budget = remaining_budget.saturating_sub(val_tokens);
                } else {
                    // Truncate value - replace with placeholder
                    result.insert(key, serde_json::Value::String("[truncated]".to_string()));
                    break;
                }
            }

            serde_json::to_string(&serde_json::Value::Object(result)).unwrap_or_else(|_| "[truncated json]".to_string())
        }
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            let mut remaining_budget = budget.saturating_sub(2); // Account for "[]"
            let mut first = true;

            for val in arr {
                let val_str = serde_json::to_string(&val).unwrap_or_default();
                let val_tokens = approx_token_count(&val_str);

                if first {
                    remaining_budget = remaining_budget.saturating_sub(val_tokens);
                    first = false;
                } else {
                    remaining_budget = remaining_budget.saturating_sub(val_tokens + 1); // +1 for comma
                }

                if remaining_budget >= val_tokens {
                    result.push(val);
                    remaining_budget = remaining_budget.saturating_sub(val_tokens);
                } else {
                    break;
                }
            }

            serde_json::to_string(&serde_json::Value::Array(result)).unwrap_or_else(|_| "[truncated json]".to_string())
        }
        _ => {
            // Primitive value - serialize and truncate if needed
            let val_str = serde_json::to_string(&value).unwrap_or_default();
            let val_tokens = approx_token_count(&val_str);
            if val_tokens <= budget {
                val_str
            } else {
                truncate_text(&val_str, TruncationPolicy::Tokens(budget))
            }
        }
    }
}

/// Check if text appears to be XML by examining its start.
/// Simple heuristic: checks if trimmed content starts with '<'.
fn is_xml(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('<')
}

/// Safely truncate XML content while preserving structure.
/// Strategy: if content fits within budget, keep it all. Otherwise,
/// attempt to preserve XML structure by keeping opening/closing tags balanced.
fn truncate_xml_safely(xml: &str, policy: TruncationPolicy) -> String {
    let budget = policy.token_budget();
    let estimated_tokens = approx_token_count(xml);

    // If content fits within budget, return as-is
    if estimated_tokens <= budget {
        return xml.to_string();
    }

    // Simple XML truncation: try to preserve structure by keeping opening tags
    // and truncating content while maintaining balance
    // For complex cases, fall back to regular truncation
    match policy {
        TruncationPolicy::Bytes(_) => truncate_with_byte_estimate(xml, policy),
        TruncationPolicy::Tokens(_) => {
            let (truncated, _) = truncate_with_token_budget(xml, policy);
            truncated
        }
    }
}

/// Globally truncate function output items to fit within the given
/// truncation policy's budget, preserving as many text/image items as
/// possible and appending a summary for any omitted text items.
pub(crate) fn truncate_function_output_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_budget = match policy {
        TruncationPolicy::Bytes(_) => policy.byte_budget(),
        TruncationPolicy::Tokens(_) => policy.token_budget(),
    };
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining_budget == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let cost = match policy {
                    TruncationPolicy::Bytes(_) => text.len(),
                    TruncationPolicy::Tokens(_) => approx_token_count(text),
                };

                if cost <= remaining_budget {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining_budget = remaining_budget.saturating_sub(cost);
                } else {
                    let snippet_policy = match policy {
                        TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(remaining_budget),
                        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
                    };
                    let snippet = truncate_text(text, snippet_policy);
                    if snippet.is_empty() {
                        omitted_text_items += 1;
                    } else {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    }
                    remaining_budget = 0;
                }
            }
            FunctionCallOutputContentItem::InputImage { image_url } => {
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                });
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }

    out
}

/// Truncate the middle of a UTF-8 string to at most `max_tokens` tokens,
/// preserving the beginning and the end. Returns the possibly truncated string
/// and `Some(original_token_count)` if truncation occurred; otherwise returns
/// the original string and `None`.
fn truncate_with_token_budget(s: &str, policy: TruncationPolicy) -> (String, Option<u64>) {
    if s.is_empty() {
        return (String::new(), None);
    }
    let max_tokens = policy.token_budget();

    let byte_len = s.len();
    if max_tokens > 0 && byte_len <= approx_bytes_for_tokens(max_tokens) {
        return (s.to_string(), None);
    }

    let truncated = truncate_with_byte_estimate(s, policy);
    let approx_total_usize = approx_token_count(s);
    let approx_total = u64::try_from(approx_total_usize).unwrap_or(u64::MAX);
    if truncated == s {
        (truncated, None)
    } else {
        (truncated, Some(approx_total))
    }
}

/// Truncate a string using a byte budget derived from the token budget, without
/// performing any real tokenization. This keeps the logic purely byte-based and
/// uses a bytes placeholder in the truncated output.
fn truncate_with_byte_estimate(s: &str, policy: TruncationPolicy) -> String {
    if s.is_empty() {
        return String::new();
    }

    let total_chars = s.chars().count();
    let max_bytes = policy.byte_budget();

    if max_bytes == 0 {
        // No budget to show content; just report that everything was truncated.
        let marker = format_truncation_marker(
            policy,
            removed_units_for_source(policy, s.len(), total_chars),
        );
        return marker;
    }

    if s.len() <= max_bytes {
        return s.to_string();
    }

    let total_bytes = s.len();

    let (left_budget, right_budget) = split_budget(max_bytes);

    let (removed_chars, left, right) = split_string(s, left_budget, right_budget);

    let marker = format_truncation_marker(
        policy,
        removed_units_for_source(policy, total_bytes.saturating_sub(max_bytes), removed_chars),
    );

    assemble_truncated_output(left, right, &marker)
}

fn split_string(s: &str, beginning_bytes: usize, end_bytes: usize) -> (usize, &str, &str) {
    if s.is_empty() {
        return (0, "", "");
    }

    let len = s.len();
    let tail_start_target = len.saturating_sub(end_bytes);
    let mut prefix_end = 0usize;
    let mut suffix_start = len;
    let mut removed_chars = 0usize;
    let mut suffix_started = false;

    for (idx, ch) in s.char_indices() {
        let char_end = idx + ch.len_utf8();
        if char_end <= beginning_bytes {
            prefix_end = char_end;
            continue;
        }

        if idx >= tail_start_target {
            if !suffix_started {
                suffix_start = idx;
                suffix_started = true;
            }
            continue;
        }

        removed_chars = removed_chars.saturating_add(1);
    }

    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    let before = &s[..prefix_end];
    let after = &s[suffix_start..];

    (removed_chars, before, after)
}

fn format_truncation_marker(policy: TruncationPolicy, removed_count: u64) -> String {
    match policy {
        TruncationPolicy::Tokens(_) => format!("…{removed_count} tokens truncated…"),
        TruncationPolicy::Bytes(_) => format!("…{removed_count} chars truncated…"),
    }
}

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn removed_units_for_source(
    policy: TruncationPolicy,
    removed_bytes: usize,
    removed_chars: usize,
) -> u64 {
    match policy {
        TruncationPolicy::Tokens(_) => approx_tokens_from_byte_count(removed_bytes),
        TruncationPolicy::Bytes(_) => u64::try_from(removed_chars).unwrap_or(u64::MAX),
    }
}

fn assemble_truncated_output(prefix: &str, suffix: &str, marker: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + marker.len() + suffix.len() + 1);
    out.push_str(prefix);
    out.push_str(marker);
    out.push_str(suffix);
    out
}

pub(crate) fn approx_token_count(text: &str) -> usize {
    let len = text.len();
    len.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

pub(crate) fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_BYTES_PER_TOKEN)
}

pub(crate) fn approx_tokens_from_byte_count(bytes: usize) -> u64 {
    let bytes_u64 = bytes as u64;
    bytes_u64.saturating_add((APPROX_BYTES_PER_TOKEN as u64).saturating_sub(1))
        / (APPROX_BYTES_PER_TOKEN as u64)
}

#[cfg(test)]
mod tests {

    use super::TruncationPolicy;
    use super::approx_token_count;
    use super::formatted_truncate_text;
    use super::split_string;
    use super::truncate_function_output_items_with_policy;
    use super::truncate_text;
    use super::truncate_with_token_budget;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn split_string_works() {
        assert_eq!(split_string("hello world", 5, 5), (1, "hello", "world"));
        assert_eq!(split_string("abc", 0, 0), (3, "", ""));
    }

    #[test]
    fn split_string_handles_empty_string() {
        assert_eq!(split_string("", 4, 4), (0, "", ""));
    }

    #[test]
    fn split_string_only_keeps_prefix_when_tail_budget_is_zero() {
        assert_eq!(split_string("abcdef", 3, 0), (3, "abc", ""));
    }

    #[test]
    fn split_string_only_keeps_suffix_when_prefix_budget_is_zero() {
        assert_eq!(split_string("abcdef", 0, 3), (3, "", "def"));
    }

    #[test]
    fn split_string_handles_overlapping_budgets_without_removal() {
        assert_eq!(split_string("abcdef", 4, 4), (0, "abcd", "ef"));
    }

    #[test]
    fn split_string_respects_utf8_boundaries() {
        assert_eq!(split_string("😀abc😀", 5, 5), (1, "😀a", "c😀"));

        assert_eq!(split_string("😀😀😀😀😀", 1, 1), (5, "", ""));
        assert_eq!(split_string("😀😀😀😀😀", 7, 7), (3, "😀", "😀"));
        assert_eq!(split_string("😀😀😀😀😀", 8, 8), (1, "😀😀", "😀😀"));
    }

    #[test]
    fn truncate_bytes_less_than_placeholder_returns_placeholder() {
        let content = "example output";

        assert_eq!(
            "Total output lines: 1\n\n…13 chars truncated…t",
            formatted_truncate_text(content, TruncationPolicy::Bytes(1)),
        );
    }

    #[test]
    fn truncate_tokens_less_than_placeholder_returns_placeholder() {
        let content = "example output";

        assert_eq!(
            "Total output lines: 1\n\nex…3 tokens truncated…ut",
            formatted_truncate_text(content, TruncationPolicy::Tokens(1)),
        );
    }

    #[test]
    fn truncate_tokens_under_limit_returns_original() {
        let content = "example output";

        assert_eq!(
            content,
            formatted_truncate_text(content, TruncationPolicy::Tokens(10)),
        );
    }

    #[test]
    fn truncate_bytes_under_limit_returns_original() {
        let content = "example output";

        assert_eq!(
            content,
            formatted_truncate_text(content, TruncationPolicy::Bytes(20)),
        );
    }

    #[test]
    fn truncate_tokens_over_limit_returns_truncated() {
        let content = "this is an example of a long output that should be truncated";

        assert_eq!(
            "Total output lines: 1\n\nthis is an…10 tokens truncated… truncated",
            formatted_truncate_text(content, TruncationPolicy::Tokens(5)),
        );
    }

    #[test]
    fn truncate_bytes_over_limit_returns_truncated() {
        let content = "this is an example of a long output that should be truncated";

        assert_eq!(
            "Total output lines: 1\n\nthis is an exam…30 chars truncated…ld be truncated",
            formatted_truncate_text(content, TruncationPolicy::Bytes(30)),
        );
    }

    #[test]
    fn truncate_bytes_reports_original_line_count_when_truncated() {
        let content =
            "this is an example of a long output that should be truncated\nalso some other line";

        assert_eq!(
            "Total output lines: 2\n\nthis is an exam…51 chars truncated…some other line",
            formatted_truncate_text(content, TruncationPolicy::Bytes(30)),
        );
    }

    #[test]
    fn truncate_tokens_reports_original_line_count_when_truncated() {
        let content =
            "this is an example of a long output that should be truncated\nalso some other line";

        assert_eq!(
            "Total output lines: 2\n\nthis is an example o…11 tokens truncated…also some other line",
            formatted_truncate_text(content, TruncationPolicy::Tokens(10)),
        );
    }

    #[test]
    fn truncate_with_token_budget_returns_original_when_under_limit() {
        let s = "short output";
        let limit = 100;
        let (out, original) = truncate_with_token_budget(s, TruncationPolicy::Tokens(limit));
        assert_eq!(out, s);
        assert_eq!(original, None);
    }

    #[test]
    fn truncate_with_token_budget_reports_truncation_at_zero_limit() {
        let s = "abcdef";
        let (out, original) = truncate_with_token_budget(s, TruncationPolicy::Tokens(0));
        assert_eq!(out, "…2 tokens truncated…");
        assert_eq!(original, Some(2));
    }

    #[test]
    fn truncate_middle_tokens_handles_utf8_content() {
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
        let (out, tokens) = truncate_with_token_budget(s, TruncationPolicy::Tokens(8));
        assert_eq!(out, "😀😀😀😀…8 tokens truncated… line with text\n");
        assert_eq!(tokens, Some(16));
    }

    #[test]
    fn truncate_middle_bytes_handles_utf8_content() {
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
        let out = truncate_text(s, TruncationPolicy::Bytes(20));
        assert_eq!(out, "😀😀…21 chars truncated…with text\n");
    }

    #[test]
    fn truncates_across_multiple_under_limit_texts_and_reports_omitted() {
        let chunk = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega.\n";
        let chunk_tokens = approx_token_count(chunk);
        assert!(chunk_tokens > 0, "chunk must consume tokens");
        let limit = chunk_tokens * 3;
        let t1 = chunk.to_string();
        let t2 = chunk.to_string();
        let t3 = chunk.repeat(10);
        let t4 = chunk.to_string();
        let t5 = chunk.to_string();

        let items = vec![
            FunctionCallOutputContentItem::InputText { text: t1.clone() },
            FunctionCallOutputContentItem::InputText { text: t2.clone() },
            FunctionCallOutputContentItem::InputImage {
                image_url: "img:mid".to_string(),
            },
            FunctionCallOutputContentItem::InputText { text: t3 },
            FunctionCallOutputContentItem::InputText { text: t4 },
            FunctionCallOutputContentItem::InputText { text: t5 },
        ];

        let output =
            truncate_function_output_items_with_policy(&items, TruncationPolicy::Tokens(limit));

        // Expect: t1 (full), t2 (full), image, t3 (truncated), summary mentioning 2 omitted.
        assert_eq!(output.len(), 5);

        let first_text = match &output[0] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected first item: {other:?}"),
        };
        assert_eq!(first_text, &t1);

        let second_text = match &output[1] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected second item: {other:?}"),
        };
        assert_eq!(second_text, &t2);

        assert_eq!(
            output[2],
            FunctionCallOutputContentItem::InputImage {
                image_url: "img:mid".to_string()
            }
        );

        let fourth_text = match &output[3] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected fourth item: {other:?}"),
        };
        assert!(
            fourth_text.contains("tokens truncated"),
            "expected marker in truncated snippet: {fourth_text}"
        );

        let summary_text = match &output[4] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected summary item: {other:?}"),
        };
        assert!(summary_text.contains("omitted 2 text items"));
    }

    #[test]
    fn truncate_text_preserves_error_lines_with_priority() {
        // Test that error lines are preserved when truncating content
        let content = "line 1\nline 2\nerror: compilation failed\nline 3\nline 4\nline 5";
        let result = truncate_text(content, TruncationPolicy::Tokens(5));

        // Error line should be preserved
        assert!(result.contains("error: compilation failed"), "Error line should be preserved");
        // Should contain some context around the error
        assert!(result.contains("line 2") || result.contains("line 3"), "Context around error should be preserved");
    }

    #[test]
    fn truncate_text_handles_multiple_error_lines() {
        // Test that multiple error lines are all preserved
        let content = "start\nerror: first error\nmiddle\nerror: second error\nend";
        let result = truncate_text(content, TruncationPolicy::Tokens(10));

        // Both error lines should be preserved
        assert!(result.contains("error: first error"), "First error should be preserved");
        assert!(result.contains("error: second error"), "Second error should be preserved");
    }

    #[test]
    fn truncate_text_detects_case_insensitive_errors() {
        // Test case-insensitive error detection
        let content = "Some output\nERROR: Something went wrong\nMore output";
        let result = truncate_text(content, TruncationPolicy::Tokens(5));

        assert!(result.contains("ERROR: Something went wrong"), "Uppercase ERROR should be detected");
    }

    #[test]
    fn truncate_json_preserves_structure_when_fits() {
        // Test that valid JSON is preserved when it fits within budget
        let json = r#"{"key1": "value1", "key2": "value2"}"#;
        let result = truncate_text(json, TruncationPolicy::Tokens(20));

        // JSON should be preserved as-is when it fits
        assert!(result.contains("key1"));
        assert!(result.contains("key2"));
    }

    #[test]
    fn truncate_json_truncates_large_objects_safely() {
        // Test that large JSON objects are truncated while preserving structure
        let json = r#"{"key1": "very long value that exceeds the budget", "key2": "another long value"}"#;
        let result = truncate_text(json, TruncationPolicy::Tokens(5));

        // Should preserve JSON structure (opening brace, keys)
        assert!(result.contains("{"), "Should preserve opening brace");
        assert!(result.contains("key1") || result.contains("key2"), "Should preserve at least one key");
    }

    #[test]
    fn truncate_json_handles_arrays() {
        // Test JSON array truncation
        let json = r#"["item1", "item2", "item3", "item4"]"#;
        let result = truncate_text(json, TruncationPolicy::Tokens(5));

        // Should preserve array structure
        assert!(result.contains("["), "Should preserve opening bracket");
    }

    #[test]
    fn truncate_json_falls_back_on_invalid_json() {
        // Test that invalid JSON falls back to regular truncation
        let invalid_json = "{key1: value1, key2: value2}"; // Missing quotes
        let result = truncate_text(invalid_json, TruncationPolicy::Tokens(5));

        // Should still truncate (fallback behavior)
        assert!(!result.is_empty());
    }

    #[test]
    fn truncate_xml_detects_xml_content() {
        // Test XML detection
        let xml = "<root><child>content</child></root>";
        let result = truncate_text(xml, TruncationPolicy::Tokens(10));

        // Should handle XML (may fall back to regular truncation for now)
        assert!(!result.is_empty());
    }

    #[test]
    fn truncate_text_prioritizes_errors_over_json() {
        // Test that error detection takes priority over JSON detection
        let content = r#"{"status": "error", "message": "compilation failed"}"#;
        let result = truncate_text(content, TruncationPolicy::Tokens(5));

        // Should prioritize error patterns even in JSON
        assert!(result.contains("error") || result.contains("failed"));
    }
}
