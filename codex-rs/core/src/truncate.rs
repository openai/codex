//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries, and helpers for line/token‑based truncation
//! used across the core crate.

use crate::config::Config;
use codex_protocol::models::FunctionCallOutputContentItem;

const APPROX_BYTES_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl TruncationPolicy {
    pub fn new(config: &Config) -> Self {
        let config_token_limit = config.tool_output_token_limit;

        match config.model_family.truncation_policy {
            TruncationPolicy::Bytes(family_bytes) => {
                if let Some(token_limit) = config_token_limit {
                    Self::Bytes(approx_bytes_for_tokens(token_limit))
                } else {
                    Self::Bytes(family_bytes)
                }
            }
            TruncationPolicy::Tokens(family_tokens) => {
                if let Some(token_limit) = config_token_limit {
                    Self::Tokens(token_limit)
                } else {
                    Self::Tokens(family_tokens)
                }
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

pub(crate) fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    match policy {
        TruncationPolicy::Bytes(bytes) => truncate_with_byte_estimate(
            content,
            bytes,
            TruncationSource::Policy(TruncationPolicy::Bytes(bytes)),
        ),
        TruncationPolicy::Tokens(tokens) => {
            let (truncated, _) = truncate_with_token_budget(
                content,
                tokens,
                TruncationSource::Policy(TruncationPolicy::Tokens(tokens)),
            );
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
fn truncate_with_token_budget(
    s: &str,
    max_tokens: usize,
    source: TruncationSource,
) -> (String, Option<u64>) {
    if s.is_empty() {
        return (String::new(), None);
    }

    let byte_len = s.len();
    if max_tokens > 0 {
        let small_threshold = approx_bytes_for_tokens(max_tokens / 4);
        if small_threshold > 0 && byte_len <= small_threshold {
            return (s.to_string(), None);
        }
    }

    let truncated = truncate_with_byte_estimate(s, approx_bytes_for_tokens(max_tokens), source);
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
fn truncate_with_byte_estimate(s: &str, max_bytes: usize, source: TruncationSource) -> String {
    if s.is_empty() {
        return String::new();
    }

    if max_bytes == 0 {
        // No budget to show content; just report that everything was truncated.
        let marker = format_truncation_marker(source, removed_units_for_source(source, s.len()));
        return marker;
    }

    if s.len() <= max_bytes {
        return s.to_string();
    }

    let total_bytes = s.len();
    let removed_bytes = total_bytes.saturating_sub(max_bytes);
    let marker = format_truncation_marker(source, removed_units_for_source(source, removed_bytes));
    let marker_len = marker.len();

    if marker_len >= max_bytes {
        let truncated_marker = truncate_on_boundary(&marker, max_bytes);
        return truncated_marker.to_string();
    }

    let keep_budget = max_bytes - marker_len;
    let (left_budget, right_budget) = split_budget(keep_budget);
    let prefix_end = pick_prefix_end(s, left_budget);
    let mut suffix_start = pick_suffix_start(s, right_budget);
    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    let mut out = assemble_truncated_output(&s[..prefix_end], &s[suffix_start..], &marker);

    if out.len() > max_bytes {
        let boundary = truncate_on_boundary(&out, max_bytes);
        out.truncate(boundary.len());
    }

    out
}

#[derive(Clone, Copy)]
pub enum TruncationSource {
    Policy(TruncationPolicy),
}

fn format_truncation_marker(source: TruncationSource, removed_count: u64) -> String {
    match source {
        TruncationSource::Policy(TruncationPolicy::Tokens(_)) => {
            format!("[…{removed_count} tokens truncated…]")
        }
        TruncationSource::Policy(TruncationPolicy::Bytes(_)) => {
            format!("[…{removed_count} bytes truncated…]")
        }
    }
}

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn removed_units_for_source(source: TruncationSource, removed_bytes: usize) -> u64 {
    match source {
        TruncationSource::Policy(TruncationPolicy::Tokens(_)) => {
            approx_tokens_from_byte_count(removed_bytes)
        }
        TruncationSource::Policy(TruncationPolicy::Bytes(_)) => {
            u64::try_from(removed_bytes).unwrap_or(u64::MAX)
        }
    }
}

fn assemble_truncated_output(prefix: &str, suffix: &str, marker: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + marker.len() + suffix.len() + 1);
    out.push_str(prefix);
    out.push_str(marker);
    out.push('\n');
    out.push_str(suffix);
    out
}

pub(crate) fn approx_token_count(text: &str) -> usize {
    let len = text.len();
    len.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_BYTES_PER_TOKEN)
}

fn approx_tokens_from_byte_count(bytes: usize) -> u64 {
    let bytes_u64 = bytes as u64;
    bytes_u64.saturating_add((APPROX_BYTES_PER_TOKEN as u64).saturating_sub(1))
        / (APPROX_BYTES_PER_TOKEN as u64)
}

fn truncate_on_boundary(input: &str, max_len: usize) -> &str {
    if input.len() <= max_len {
        return input;
    }
    let mut end = max_len;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

fn pick_prefix_end(s: &str, left_budget: usize) -> usize {
    if let Some(head) = s.get(..left_budget)
        && let Some(i) = head.rfind('\n')
    {
        return i + 1;
    }
    truncate_on_boundary(s, left_budget).len()
}

fn pick_suffix_start(s: &str, right_budget: usize) -> usize {
    let start_tail = s.len().saturating_sub(right_budget);
    if let Some(tail) = s.get(start_tail..)
        && let Some(i) = tail.find('\n')
    {
        return start_tail + i + 1;
    }

    let mut idx = start_tail.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use crate::config::OPENAI_DEFAULT_MODEL;
    use crate::model_family::derive_default_model_family;
    use crate::model_family::find_family_for_model;

    use super::TruncationPolicy;
    use super::TruncationSource;
    use super::approx_token_count;
    use super::truncate_function_output_items_with_policy;
    use super::truncate_text;
    use super::truncate_with_token_budget;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use pretty_assertions::assert_eq;
    use regex_lite::Regex;

    const MODEL_FORMAT_MAX_LINES: usize = 256;

    fn model_format_max_bytes() -> usize {
        find_family_for_model(OPENAI_DEFAULT_MODEL)
            .unwrap_or_else(|| derive_default_model_family(OPENAI_DEFAULT_MODEL))
            .truncation_policy
            .byte_budget()
    }

    fn truncate_model_output(content: &str) -> String {
        truncate_text(content, TruncationPolicy::Bytes(model_format_max_bytes()))
    }

    fn assert_truncated_message_matches(message: &str, line: &str, total_lines: usize) {
        let pattern = truncated_message_pattern(line, total_lines);
        let regex = Regex::new(&pattern).unwrap_or_else(|err| {
            panic!("failed to compile regex {pattern}: {err}");
        });
        let captures = regex
            .captures(message)
            .unwrap_or_else(|| panic!("message failed to match pattern {pattern}: {message}"));
        let body = captures
            .name("body")
            .expect("missing body capture")
            .as_str();
        assert!(
            body.len() <= model_format_max_bytes(),
            "body exceeds byte limit: {} bytes",
            body.len()
        );
    }

    fn truncated_message_pattern(line: &str, total_lines: usize) -> String {
        let head_lines = MODEL_FORMAT_MAX_LINES / 2;
        let tail_lines = MODEL_FORMAT_MAX_LINES - head_lines;
        let head_take = head_lines.min(total_lines);
        let tail_take = tail_lines.min(total_lines.saturating_sub(head_take));
        let omitted = total_lines.saturating_sub(head_take + tail_take);
        let escaped_line = regex_lite::escape(line);
        if omitted == 0 {
            return format!(
                r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} removed \d+ bytes to fit {max_bytes} byte limit \.{{3}}]\n\n.*)$",
                max_bytes = model_format_max_bytes(),
            );
        }
        format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
        )
    }

    #[test]
    fn truncate_middle_returns_original_when_under_limit() {
        let s = "short output";
        let limit = 100;
        let source = TruncationSource::Policy(TruncationPolicy::Tokens(limit));
        let (out, original) = truncate_with_token_budget(s, limit, source);
        assert_eq!(out, s);
        assert_eq!(original, None);
    }

    #[test]
    fn truncate_middle_reports_truncation_at_zero_limit() {
        let s = "abcdef";
        let source = TruncationSource::Policy(TruncationPolicy::Tokens(0));
        let (out, original) = truncate_with_token_budget(s, 0, source);
        assert_eq!(out, "[…2 tokens truncated…]");
        assert_eq!(original, Some(approx_token_count(s) as u64));
    }

    #[test]
    fn truncate_middle_enforces_token_budget() {
        let s = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let max_tokens = 12;
        let source = TruncationSource::Policy(TruncationPolicy::Tokens(max_tokens));
        let (out, original) = truncate_with_token_budget(s, max_tokens, source);
        assert!(out.contains("tokens truncated"));
        assert_eq!(original, Some(approx_token_count(s) as u64));
        assert!(out.len() < s.len(), "truncated output should be shorter");
    }

    #[test]
    fn truncate_middle_handles_utf8_content() {
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
        let max_tokens = 8;
        let source = TruncationSource::Policy(TruncationPolicy::Tokens(max_tokens));
        let (out, tokens) = truncate_with_token_budget(s, max_tokens, source);

        assert!(out.contains("tokens truncated"));
        assert!(!out.contains('\u{fffd}'));
        assert_eq!(tokens, Some(approx_token_count(s) as u64));
        assert!(out.len() < s.len(), "UTF-8 content should be shortened");
    }

    #[test]
    fn format_exec_output_truncates_large_error() {
        let line = "very long execution error line that should trigger truncation\n";
        let large_error = line.repeat(2_500); // way beyond both byte and line limits

        let truncated = truncate_model_output(&large_error);

        let total_lines = large_error.lines().count();
        assert_truncated_message_matches(&truncated, line, total_lines);
        assert_ne!(truncated, large_error);
    }

    #[test]
    fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
        let max_bytes = model_format_max_bytes();
        let long_line = "a".repeat(max_bytes + 50);
        let truncated = truncate_model_output(&long_line);

        assert_ne!(truncated, long_line);
        let removed_bytes = long_line.len().saturating_sub(max_bytes);
        let marker_line =
            format!("[... removed {removed_bytes} bytes to fit {max_bytes} byte limit ...]");
        assert!(
            truncated.contains(&marker_line),
            "missing byte truncation marker: {truncated}"
        );
        assert!(
            !truncated.contains("omitted"),
            "line omission marker should not appear when no lines were dropped: {truncated}"
        );
    }

    #[test]
    fn format_exec_output_returns_original_when_within_limits() {
        let content = "example output\n".repeat(10);

        assert_eq!(truncate_model_output(&content), content);
    }

    #[test]
    fn format_exec_output_preserves_head_and_tail_after_truncation() {
        let total_lines = 600;
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}\n"))
            .collect();

        let truncated = truncate_model_output(&content);
        assert!(
            truncated.contains("line-0\n"),
            "expected head line to remain: {truncated}"
        );

        let last_line = format!("line-{}\n", total_lines - 1);
        assert!(
            truncated.contains(&last_line),
            "expected tail line to remain: {truncated}"
        );
        assert!(
            truncated.contains("bytes truncated"),
            "expected truncation marker: {truncated}"
        );
    }

    #[test]
    fn format_exec_output_prefers_line_marker_when_both_limits_exceeded() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 42;
        let long_line = "x".repeat(256);
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}-{long_line}\n"))
            .collect();

        let truncated = truncate_model_output(&content);

        assert!(
            truncated.contains("[... omitted 42 of 298 lines ...]"),
            "expected omitted marker when line count exceeds limit: {truncated}"
        );
        assert!(
            !truncated.contains("byte limit"),
            "line omission marker should take precedence over byte marker: {truncated}"
        );
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
}
