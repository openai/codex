//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries, and helpers for line/token‑based truncation
//! used across the core crate.

use std::sync::Arc;

use codex_protocol::models::FunctionCallOutputContentItem;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;
use codex_utils_tokenizer::Tokenizer;

use crate::config::Config;
use crate::model_family::derive_default_model_family;
use crate::model_family::find_family_for_model;

/// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
const TOKENIZER_STACK_SAFE_BYTES: usize = 1024 * 1024; // 1 MiB
const APPROX_BYTES_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl TruncationPolicy {
    pub fn new(config: &Config) -> Self {
        let token_limit = config.calls_output_max_tokens.unwrap_or_else(
            find_family_for_model(config.model.as_str())
                .unwrap_or_else(|| derive_default_model_family(config.model.as_str()))
                .truncation_policy,
        );

        match config.model_family.truncation_policy {
            TruncationPolicy::Bytes(_) => {
                Self::Bytes(token_limit.saturating_mul(APPROX_BYTES_PER_TOKEN))
            }
            TruncationPolicy::Tokens(_) => Self::Tokens(token_limit),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TruncationSettings {
    pub policy: TruncationPolicy,
    pub tokenizer: Arc<Option<Tokenizer>>,
}

impl TruncationSettings {
    pub fn new(policy: TruncationPolicy, model: &str) -> Self {
        let tokenizer = Arc::new(Tokenizer::for_model(model).ok());
        Self { policy, tokenizer }
    }
}

/// Format a block of exec/tool output for model consumption, truncating by
/// lines and bytes while preserving head and tail segments.
pub(crate) fn truncate_with_line_bytes_budget(content: &str, bytes_budget: usize) -> String {
    // TODO(aibrahim): to be removed
    let lines_budget = 256;
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= bytes_budget && total_lines <= lines_budget {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines, bytes_budget, lines_budget);
    format!("Total output lines: {total_lines}\n\n{output}")
}

pub(crate) fn truncate_text(
    content: &str,
    truncation_settings: &TruncationSettings,
) -> (String, Option<u64>) {
    let mode = find_family_for_model(model)
        .unwrap_or_else(|| derive_default_model_family(model))
        .truncation_policy
        .mode;
    match mode {
        TruncationMode::Bytes => truncate_with_byte_estimate(content, tokens_budget, model),
        TruncationMode::Tokens => truncate_with_token_budget(content, tokens_budget, model),
    }
}

/// Globally truncate function output items to fit within
/// `max_tokens` tokens by preserving as many
/// text/image items as possible and appending a summary for any omitted text
/// items.
pub(crate) fn truncate_function_output_items_to_token_limit(
    items: &[FunctionCallOutputContentItem],
    truncation_settings: &TruncationSettings,
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_tokens = max_tokens;
    let mut omitted_text_items = 0usize;
    let tokenizer = Tokenizer::try_default().ok();

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining_tokens == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let token_len = estimate_safe_token_count(text, tokenizer.as_ref());
                if token_len <= remaining_tokens {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining_tokens = remaining_tokens.saturating_sub(token_len);
                } else {
                    let (snippet, _) = truncate_text(text, remaining_tokens, model);
                    if snippet.is_empty() {
                        omitted_text_items += 1;
                    } else {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    }
                    remaining_tokens = 0;
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
fn truncate_with_token_budget(s: &str, max_tokens: usize, model: &str) -> (String, Option<u64>) {
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

    let exceeds_stack_limit = byte_len > TOKENIZER_STACK_SAFE_BYTES;
    let exceeds_large_threshold =
        max_tokens > 0 && byte_len > approx_bytes_for_tokens(max_tokens.saturating_mul(2));
    if exceeds_stack_limit || exceeds_large_threshold {
        return truncate_with_byte_estimate(s, max_tokens, model);
    }

    let tokenizer = match select_tokenizer(model) {
        Some(tok) => tok,
        None => return truncate_with_byte_estimate(s, max_tokens, model),
    };
    let encoded = tokenizer.encode(s, false);
    let total_tokens = encoded.len() as u64;
    truncate_with_tokenizer_path(tokenizer, encoded, max_tokens, s, total_tokens)
}

fn truncate_with_tokenizer_path(
    tokenizer: Tokenizer,
    encoded: Vec<i32>,
    max_budget: usize,
    original: &str,
    total_tokens: u64,
) -> (String, Option<u64>) {
    if max_budget == 0 {
        return (format_truncation_marker(total_tokens), Some(total_tokens));
    }

    if encoded.len() <= max_budget {
        return (original.to_string(), None);
    }

    let mut guess_removed = total_tokens.saturating_sub(max_budget as u64).max(1);
    for _ in 0..4 {
        let marker = format_truncation_marker(guess_removed);
        let marker_len = usize::try_from(tokenizer.count(&marker)).unwrap_or(usize::MAX);
        if marker_len >= max_budget {
            return (marker, Some(total_tokens));
        }

        let keep_budget = max_budget - marker_len;
        if keep_budget == 0 {
            return (marker, Some(total_tokens));
        }

        let (left_keep, right_keep) = split_budget(keep_budget);
        let removed_tokens = encoded.len().saturating_sub(left_keep + right_keep) as u64;
        let final_marker = format_truncation_marker(removed_tokens);
        let final_marker_len =
            usize::try_from(tokenizer.count(&final_marker)).unwrap_or(usize::MAX);
        if final_marker_len == marker_len {
            let (prefix, suffix) =
                decode_token_segments(&tokenizer, &encoded, left_keep, right_keep);
            let out = assemble_truncated_output(
                &prefix,
                &suffix,
                &final_marker,
                NewlineMode::WhenSuffixPresent,
            );
            return (out, Some(total_tokens));
        }

        guess_removed = removed_tokens.max(1);
    }

    let marker = format_truncation_marker(guess_removed);
    let marker_len = usize::try_from(tokenizer.count(&marker)).unwrap_or(usize::MAX);
    if marker_len >= max_budget {
        return (marker, Some(total_tokens));
    }

    let keep_budget = max_budget - marker_len;
    if keep_budget == 0 {
        return (marker, Some(total_tokens));
    }
    let (left_keep, right_keep) = split_budget(keep_budget);
    let (prefix, suffix) = decode_token_segments(&tokenizer, &encoded, left_keep, right_keep);
    let out = assemble_truncated_output(&prefix, &suffix, &marker, NewlineMode::WhenSuffixPresent);
    (out, Some(total_tokens))
}

/// Truncate a string using a byte budget derived from the token budget, without
/// performing any real tokenization. This keeps the logic purely byte-based and
/// uses a bytes placeholder in the truncated output.
fn truncate_with_byte_estimate(s: &str, max_tokens: usize, _model: &str) -> (String, Option<u64>) {
    if s.is_empty() {
        return (String::new(), None);
    }

    let total_tokens = approx_token_count(s);
    let max_bytes = approx_bytes_for_tokens(max_tokens);

    if max_bytes == 0 {
        // No budget to show content; just report that everything was truncated.
        let marker = format!("[…{} bytes truncated…]", s.len());
        return (marker, Some(total_tokens));
    }

    if s.len() <= max_bytes {
        return (s.to_string(), None);
    }

    let total_bytes = s.len();
    let removed_bytes = total_bytes.saturating_sub(max_bytes);
    let marker = format!("[…{removed_bytes} bytes truncated…]");
    let marker_len = marker.len();

    if marker_len >= max_bytes {
        let truncated_marker = truncate_on_boundary(&marker, max_bytes);
        return (truncated_marker.to_string(), Some(total_tokens));
    }

    let keep_budget = max_bytes - marker_len;
    let (left_budget, right_budget) = split_budget(keep_budget);
    let prefix_end = pick_prefix_end(s, left_budget);
    let mut suffix_start = pick_suffix_start(s, right_budget);
    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    let mut out = assemble_truncated_output(
        &s[..prefix_end],
        &s[suffix_start..],
        &marker,
        NewlineMode::Always,
    );

    if out.len() > max_bytes {
        let boundary = truncate_on_boundary(&out, max_bytes);
        out.truncate(boundary.len());
    }

    (out, Some(total_tokens))
}

fn truncate_formatted_exec_output(
    content: &str,
    total_lines: usize,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    error_on_double_truncation(content);
    let head_lines: usize = limit_lines / 2;
    let tail_lines: usize = limit_lines - head_lines; // 128
    let head_bytes: usize = limit_bytes / 2;
    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = head_lines.min(segments.len());
    let tail_take = tail_lines.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();
    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };
    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];
    let truncated_by_bytes = content.len() > limit_bytes;
    // this is a bit wrong. We are counting metadata lines and not just shell output lines.
    let marker = if omitted > 0 {
        Some(format!(
            "\n[... omitted {omitted} of {total_lines} lines ...]\n\n"
        ))
    } else if truncated_by_bytes {
        Some(format!(
            "\n[... output truncated to fit {limit_bytes} bytes ...]\n\n"
        ))
    } else {
        None
    };

    let marker_len = marker.as_ref().map_or(0, String::len);
    let base_head_budget = head_bytes.min(limit_bytes);
    let head_budget = base_head_budget.min(limit_bytes.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);
    let mut result = String::with_capacity(limit_bytes.min(content.len()));

    result.push_str(head_part);
    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = limit_bytes.saturating_sub(result.len());
    if remaining == 0 {
        return result;
    }

    let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
    result.push_str(tail_part);

    result
}

#[derive(Clone, Copy)]
enum NewlineMode {
    Always,
    WhenSuffixPresent,
}

fn format_truncation_marker(removed_tokens: u64) -> String {
    format!("[…{removed_tokens} tokens truncated…]")
}

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn decode_token_segments(
    tokenizer: &Tokenizer,
    encoded: &[i32],
    left_keep: usize,
    right_keep: usize,
) -> (String, String) {
    let prefix = if left_keep > 0 {
        tokenizer.decode(&encoded[..left_keep]).unwrap_or_default()
    } else {
        String::new()
    };
    let suffix = if right_keep > 0 {
        tokenizer
            .decode(&encoded[encoded.len() - right_keep..])
            .unwrap_or_default()
    } else {
        String::new()
    };
    (prefix, suffix)
}

fn assemble_truncated_output(
    prefix: &str,
    suffix: &str,
    marker: &str,
    newline_mode: NewlineMode,
) -> String {
    let newline_needed = match newline_mode {
        NewlineMode::Always => true,
        NewlineMode::WhenSuffixPresent => !suffix.is_empty(),
    };
    let newline_len = if newline_needed { 1 } else { 0 };
    let mut out = String::with_capacity(prefix.len() + marker.len() + suffix.len() + newline_len);
    out.push_str(prefix);
    out.push_str(marker);
    if newline_needed {
        out.push('\n');
    }
    if !suffix.is_empty() {
        out.push_str(suffix);
    }
    out
}

fn ensure_candidate_within_token_budget(
    candidate: String,
    max_budget: usize,
    total_tokens: u64,
    model: &str,
) -> (String, Option<u64>) {
    if max_budget == 0 {
        return (candidate, Some(total_tokens));
    }

    if let Some(tokenizer) = select_tokenizer(model) {
        let encoded = tokenizer.encode(candidate.as_str(), false);
        if encoded.len() > max_budget {
            return truncate_with_tokenizer_path(
                tokenizer,
                encoded,
                max_budget,
                candidate.as_str(),
                total_tokens,
            );
        }
    }

    (candidate, Some(total_tokens))
}

fn approx_token_count(text: &str) -> u64 {
    (text.len() as u64).saturating_add(3) / 4
}

fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_BYTES_PER_TOKEN)
}

fn select_tokenizer(model: &str) -> Option<Tokenizer> {
    Tokenizer::for_model(model)
        .or_else(|_| Tokenizer::try_default())
        .ok()
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

fn error_on_double_truncation(content: &str) {
    if content.contains("Total output lines:") && content.contains("omitted") {
        tracing::error!(
            "FunctionCallOutput content was already truncated before ContextManager::record_items; this would cause double truncation {content}"
        );
    }
}

fn estimate_safe_token_count(text: &str, tokenizer: Option<&Tokenizer>) -> usize {
    if text.is_empty() {
        return 0;
    }

    if text.len() > TOKENIZER_STACK_SAFE_BYTES {
        return usize::try_from(approx_token_count(text)).unwrap_or(usize::MAX);
    }

    tokenizer
        .map(|tok| usize::try_from(tok.count(text)).unwrap_or(usize::MAX))
        .unwrap_or_else(|| usize::try_from(approx_token_count(text)).unwrap_or(usize::MAX))
}

#[cfg(test)]
mod tests {
    use crate::config::OPENAI_DEFAULT_MODEL;
    use crate::model_family::derive_default_model_family;
    use crate::model_family::find_family_for_model;

    use super::truncate_function_output_items_to_token_limit;
    use super::truncate_with_line_bytes_budget;
    use super::truncate_with_token_budget;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_utils_tokenizer::Tokenizer;
    use pretty_assertions::assert_eq;
    use regex_lite::Regex;

    const MODEL_FORMAT_MAX_LINES: usize = 256;

    fn model_format_max_bytes() -> usize {
        find_family_for_model(OPENAI_DEFAULT_MODEL)
            .unwrap_or_else(|| derive_default_model_family(OPENAI_DEFAULT_MODEL))
            .truncation_policy
            .tokens_budget
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
                r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} output truncated to fit {max_bytes} bytes \.{{3}}]\n\n.*)$",
                max_bytes = model_format_max_bytes(),
            );
        }
        format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
        )
    }

    fn build_chunked_text(
        chunk: &str,
        chunk_tokens: usize,
        target_tokens: usize,
    ) -> (String, usize) {
        let mut text = String::new();
        let mut tokens = 0;
        while tokens + chunk_tokens <= target_tokens {
            text.push_str(chunk);
            tokens += chunk_tokens;
        }
        if text.is_empty() {
            text.push_str(chunk);
            tokens = chunk_tokens;
        }
        (text, tokens)
    }

    #[test]
    fn truncate_middle_returns_original_when_under_limit() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let s = "short output";
        let limit = usize::try_from(tok.count(s)).unwrap_or(0) + 10;
        let (out, original) = truncate_with_token_budget(s, limit, OPENAI_DEFAULT_MODEL);
        assert_eq!(out, s);
        assert_eq!(original, None);
    }

    #[test]
    fn truncate_middle_reports_truncation_at_zero_limit() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let s = "abcdef";
        let total = tok.count(s) as u64;
        let (out, original) = truncate_with_token_budget(s, 0, OPENAI_DEFAULT_MODEL);
        assert!(out.contains("tokens truncated"));
        assert_eq!(original, Some(total));
    }

    #[test]
    fn truncate_middle_enforces_token_budget() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let s = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let max_tokens = 12;
        let (out, original) = truncate_with_token_budget(s, max_tokens, OPENAI_DEFAULT_MODEL);
        assert!(out.contains("tokens truncated"));
        assert_eq!(original, Some(tok.count(s) as u64));
        let result_tokens = tok.count(&out) as usize;
        assert!(result_tokens <= max_tokens);
    }

    #[test]
    fn truncate_middle_handles_utf8_content() {
        let tok = Tokenizer::for_model(OPENAI_DEFAULT_MODEL).expect("load tokenizer");
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with text\n";
        let max_tokens = 8;
        let (out, tokens) = truncate_with_token_budget(s, max_tokens, OPENAI_DEFAULT_MODEL);

        assert!(out.contains("tokens truncated"));
        assert!(!out.contains('\u{fffd}'));
        assert_eq!(tokens, Some(tok.count(s) as u64));
        let result_tokens = tok.count(&out) as usize;
        assert!(result_tokens <= max_tokens);
    }

    #[test]
    fn format_exec_output_truncates_large_error() {
        let line = "very long execution error line that should trigger truncation\n";
        let large_error = line.repeat(2_500); // way beyond both byte and line limits

        let truncated = truncate_with_line_bytes_budget(&large_error, model_format_max_bytes());

        let total_lines = large_error.lines().count();
        let pattern = truncated_message_pattern(line, total_lines);
        let regex = Regex::new(&pattern).unwrap_or_else(|err| {
            panic!("failed to compile regex {pattern}: {err}");
        });
        let captures = regex
            .captures(&truncated)
            .unwrap_or_else(|| panic!("message failed to match pattern {pattern}: {truncated}"));
        let body = captures
            .name("body")
            .expect("missing body capture")
            .as_str();
        assert!(
            body.len() <= model_format_max_bytes(),
            "body exceeds byte limit: {} bytes",
            body.len()
        );
        assert_ne!(truncated, large_error);
    }

    #[test]
    fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
        let max_bytes = model_format_max_bytes();
        let long_line = "a".repeat(max_bytes + 50);
        let truncated = truncate_with_line_bytes_budget(&long_line, max_bytes);

        assert_ne!(truncated, long_line);
        let marker_line = format!("[... output truncated to fit {max_bytes} bytes ...]");
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

        assert_eq!(
            truncate_with_line_bytes_budget(&content, model_format_max_bytes()),
            content
        );
    }

    #[test]
    fn format_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 100;
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}\n"))
            .collect();

        let truncated = truncate_with_line_bytes_budget(&content, model_format_max_bytes());

        let omitted = total_lines - MODEL_FORMAT_MAX_LINES;
        let expected_marker = format!("[... omitted {omitted} of {total_lines} lines ...]");

        assert!(
            truncated.contains(&expected_marker),
            "missing omitted marker: {truncated}"
        );
        assert!(
            truncated.contains("line-0\n"),
            "expected head line to remain: {truncated}"
        );

        let last_line = format!("line-{}\n", total_lines - 1);
        assert!(
            truncated.contains(&last_line),
            "expected tail line to remain: {truncated}"
        );
    }

    #[test]
    fn format_exec_output_prefers_line_marker_when_both_limits_exceeded() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 42;
        let long_line = "x".repeat(256);
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}-{long_line}\n"))
            .collect();

        let truncated = truncate_with_line_bytes_budget(&content, model_format_max_bytes());

        assert!(
            truncated.contains("[... omitted 42 of 298 lines ...]"),
            "expected omitted marker when line count exceeds limit: {truncated}"
        );
        assert!(
            !truncated.contains("output truncated to fit"),
            "line omission marker should take precedence over byte marker: {truncated}"
        );
    }

    #[test]
    fn truncates_across_multiple_under_limit_texts_and_reports_omitted() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let chunk = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega.\n";
        let chunk_tokens = usize::try_from(tok.count(chunk)).unwrap_or(usize::MAX);
        assert!(chunk_tokens > 0, "chunk must consume tokens");
        let limit = model_format_max_bytes();
        let target_each = limit.saturating_div(2).saturating_sub(chunk_tokens);
        let (t1, t1_tokens) = build_chunked_text(chunk, chunk_tokens, target_each);
        let (t2, t2_tokens) = build_chunked_text(chunk, chunk_tokens, target_each);
        let remaining_after_t1_t2 = limit.saturating_sub(t1_tokens + t2_tokens);
        assert!(
            remaining_after_t1_t2 > 0,
            "expected positive token remainder after first two items"
        );

        let repeats_for_t3 = remaining_after_t1_t2 / chunk_tokens + 2;
        let t3 = chunk.repeat(repeats_for_t3);
        let t3_tokens = usize::try_from(tok.count(&t3)).unwrap_or(usize::MAX);
        assert!(
            t3_tokens > remaining_after_t1_t2,
            "t3 must exceed remaining tokens"
        );

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

        let model = OPENAI_DEFAULT_MODEL;

        let output = truncate_function_output_items_to_token_limit(&items, limit, model);

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
        let truncated_tokens = usize::try_from(tok.count(fourth_text)).unwrap_or(usize::MAX);
        assert!(
            truncated_tokens <= remaining_after_t1_t2,
            "truncated snippet must respect remaining token budget: {truncated_tokens} > {remaining_after_t1_t2}"
        );

        let summary_text = match &output[4] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected summary item: {other:?}"),
        };
        assert!(summary_text.contains("omitted 2 text items"));
    }
}
