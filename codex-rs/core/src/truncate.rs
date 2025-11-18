//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries, and helpers for line/token‑based truncation
//! used across the core crate.

use codex_protocol::models::FunctionCallOutputContentItem;
use codex_utils_string::take_bytes_at_char_boundary;
use codex_utils_string::take_last_bytes_at_char_boundary;
use codex_utils_tokenizer::Tokenizer;

/// Model-formatting limits: clients get full streams; only content sent to the model is truncated.
pub const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub const MODEL_FORMAT_MAX_LINES: usize = 256; // lines

/// Globally truncate function output items to fit within `MODEL_FORMAT_MAX_BYTES`
/// by preserving as many text/image items as possible and appending a summary
/// for any omitted text items.
pub(crate) fn globally_truncate_function_output_items(
    items: &[FunctionCallOutputContentItem],
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining = MODEL_FORMAT_MAX_BYTES;
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let len = text.len();
                if len <= remaining {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining -= len;
                } else {
                    let slice = take_bytes_at_char_boundary(text, remaining);
                    if !slice.is_empty() {
                        out.push(FunctionCallOutputContentItem::InputText {
                            text: slice.to_string(),
                        });
                    }
                    remaining = 0;
                }
            }
            // todo(aibrahim): handle input images; resize
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

/// Format a block of exec/tool output for model consumption, truncating by
/// lines and bytes while preserving head and tail segments.
pub(crate) fn format_output_for_model_body(
    content: &str,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.
    let total_lines = content.lines().count();
    if content.len() <= limit_bytes && total_lines <= limit_lines {
        return content.to_string();
    }
    let output = truncate_formatted_exec_output(content, total_lines, limit_bytes, limit_lines);
    format!("Total output lines: {total_lines}\n\n{output}")
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

fn error_on_double_truncation(content: &str) {
    if content.contains("Total output lines:") && content.contains("omitted") {
        tracing::error!(
            "FunctionCallOutput content was already truncated before ContextManager::record_items; this would cause double truncation {content}"
        );
    }
}

/// Truncate an output string to a maximum number of “tokens”, where tokens are
/// approximated as individual `char`s. Preserves a prefix and suffix with an
/// elision marker describing how many tokens were omitted.
pub(crate) fn truncate_output_to_tokens(
    output: &str,
    max_tokens: usize,
) -> (String, Option<usize>) {
    if max_tokens == 0 {
        let total_tokens = output.chars().count();
        let message = format!("…{total_tokens} tokens truncated…");
        return (message, Some(total_tokens));
    }

    let tokens: Vec<char> = output.chars().collect();
    let total_tokens = tokens.len();
    if total_tokens <= max_tokens {
        return (output.to_string(), None);
    }

    let half = max_tokens / 2;
    if half == 0 {
        let truncated = total_tokens.saturating_sub(max_tokens);
        let message = format!("…{truncated} tokens truncated…");
        return (message, Some(total_tokens));
    }

    let truncated = total_tokens.saturating_sub(half * 2);
    let mut truncated_output = String::new();
    truncated_output.extend(&tokens[..half]);
    truncated_output.push_str(&format!("…{truncated} tokens truncated…"));
    truncated_output.extend(&tokens[total_tokens - half..]);
    (truncated_output, Some(total_tokens))
}

/// Truncate the middle of a UTF-8 string to at most `max_bytes` bytes,
/// preserving the beginning and the end. Returns the possibly truncated
/// string and `Some(original_token_count)` (counted with the local tokenizer;
/// falls back to a 4-bytes-per-token estimate if the tokenizer cannot load)
/// if truncation occurred; otherwise returns the original string and `None`.
pub(crate) fn truncate_middle(s: &str, max_bytes: usize) -> (String, Option<u64>) {
    if s.len() <= max_bytes {
        return (s.to_string(), None);
    }

    // Build a tokenizer for counting (default to o200k_base; fall back to cl100k_base).
    // If both fail, fall back to a 4-bytes-per-token estimate.
    let tok = Tokenizer::try_default().ok();
    let token_count = |text: &str| -> u64 {
        if let Some(ref t) = tok {
            t.count(text) as u64
        } else {
            (text.len() as u64).div_ceil(4)
        }
    };

    let total_tokens = token_count(s);
    if max_bytes == 0 {
        return (
            format!("…{total_tokens} tokens truncated…"),
            Some(total_tokens),
        );
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

    // Iterate to stabilize marker length → keep budget → boundaries.
    let mut guess_tokens: u64 = 1;
    for _ in 0..4 {
        let marker = format!("…{guess_tokens} tokens truncated…");
        let marker_len = marker.len();
        let keep_budget = max_bytes.saturating_sub(marker_len);
        if keep_budget == 0 {
            return (
                format!("…{total_tokens} tokens truncated…"),
                Some(total_tokens),
            );
        }

        let left_budget = keep_budget / 2;
        let right_budget = keep_budget - left_budget;
        let prefix_end = pick_prefix_end(s, left_budget);
        let mut suffix_start = pick_suffix_start(s, right_budget);
        if suffix_start < prefix_end {
            suffix_start = prefix_end;
        }

        // Tokens actually removed (middle slice) using the real tokenizer.
        let removed_tokens = token_count(&s[prefix_end..suffix_start]);

        // If the number of digits in the token count does not change the marker length,
        // we can finalize output.
        let final_marker = format!("…{removed_tokens} tokens truncated…");
        if final_marker.len() == marker_len {
            let kept_content_bytes = prefix_end + (s.len() - suffix_start);
            let mut out = String::with_capacity(final_marker.len() + kept_content_bytes + 1);
            out.push_str(&s[..prefix_end]);
            out.push_str(&final_marker);
            out.push('\n');
            out.push_str(&s[suffix_start..]);
            return (out, Some(total_tokens));
        }

        guess_tokens = removed_tokens;
    }

    // Fallback build after iterations: compute with the last guess.
    let marker = format!("…{guess_tokens} tokens truncated…");
    let marker_len = marker.len();
    let keep_budget = max_bytes.saturating_sub(marker_len);
    if keep_budget == 0 {
        return (
            format!("…{total_tokens} tokens truncated…"),
            Some(total_tokens),
        );
    }

    let left_budget = keep_budget / 2;
    let right_budget = keep_budget - left_budget;
    let prefix_end = pick_prefix_end(s, left_budget);
    let mut suffix_start = pick_suffix_start(s, right_budget);
    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    let mut out = String::with_capacity(marker_len + prefix_end + (s.len() - suffix_start) + 1);
    out.push_str(&s[..prefix_end]);
    out.push_str(&marker);
    out.push('\n');
    out.push_str(&s[suffix_start..]);
    (out, Some(total_tokens))
}

#[cfg(test)]
mod tests {
    use super::MODEL_FORMAT_MAX_BYTES;
    use super::MODEL_FORMAT_MAX_LINES;
    use super::format_output_for_model_body;
    use super::globally_truncate_function_output_items;
    use super::truncate_middle;
    use super::truncate_output_to_tokens;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_utils_tokenizer::Tokenizer;
    use pretty_assertions::assert_eq;
    use regex_lite::Regex;

    fn truncated_message_pattern(line: &str, total_lines: usize) -> String {
        let head_lines = MODEL_FORMAT_MAX_LINES / 2;
        let tail_lines = MODEL_FORMAT_MAX_LINES - head_lines;
        let head_take = head_lines.min(total_lines);
        let tail_take = tail_lines.min(total_lines.saturating_sub(head_take));
        let omitted = total_lines.saturating_sub(head_take + tail_take);
        let escaped_line = regex_lite::escape(line);
        if omitted == 0 {
            return format!(
                r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes \.{{3}}]\n\n.*)$",
            );
        }
        format!(
            r"(?s)^Total output lines: {total_lines}\n\n(?P<body>{escaped_line}.*\n\[\.{{3}} omitted {omitted} of {total_lines} lines \.{{3}}]\n\n.*)$",
        )
    }

    #[test]
    fn truncate_middle_no_newlines_fallback() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let s = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ*";
        let max_bytes = 32;
        let (out, original) = truncate_middle(s, max_bytes);
        assert!(out.starts_with("abc"));
        assert!(out.contains("tokens truncated"));
        assert!(out.ends_with("XYZ*"));
        assert_eq!(original, Some(tok.count(s) as u64));
    }

    #[test]
    fn truncate_middle_prefers_newline_boundaries() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let mut s = String::new();
        for i in 1..=20 {
            s.push_str(&format!("{i:03}\n"));
        }
        assert_eq!(s.len(), 80);

        let max_bytes = 64;
        let (out, tokens) = truncate_middle(&s, max_bytes);
        assert!(out.starts_with("001\n002\n003\n004\n"));
        assert!(out.contains("tokens truncated"));
        assert!(out.ends_with("017\n018\n019\n020\n"));
        assert_eq!(tokens, Some(tok.count(&s) as u64));
    }

    #[test]
    fn truncate_middle_handles_utf8_content() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with ascii text\n";
        let max_bytes = 32;
        let (out, tokens) = truncate_middle(s, max_bytes);

        assert!(out.contains("tokens truncated"));
        assert!(!out.contains('\u{fffd}'));
        assert_eq!(tokens, Some(tok.count(s) as u64));
    }

    #[test]
    fn truncate_middle_prefers_newline_boundaries_2() {
        let tok = Tokenizer::try_default().expect("load tokenizer");
        // Build a multi-line string of 20 numbered lines (each "NNN\n").
        let mut s = String::new();
        for i in 1..=20 {
            s.push_str(&format!("{i:03}\n"));
        }
        assert_eq!(s.len(), 80);

        let max_bytes = 64;
        let (out, total) = truncate_middle(&s, max_bytes);
        assert!(out.starts_with("001\n002\n003\n004\n"));
        assert!(out.contains("tokens truncated"));
        assert!(out.ends_with("017\n018\n019\n020\n"));
        assert_eq!(total, Some(tok.count(&s) as u64));
    }

    #[test]
    fn truncate_output_to_tokens_returns_original_when_under_limit() {
        let s = "short output";
        let (truncated, original) = truncate_output_to_tokens(s, 100);
        assert_eq!(truncated, s);
        assert_eq!(original, None);
    }

    #[test]
    fn truncate_output_to_tokens_reports_truncation_at_zero_limit() {
        let s = "abcdef";
        let (truncated, original) = truncate_output_to_tokens(s, 0);
        assert!(truncated.contains("tokens truncated"));
        assert_eq!(original, Some(s.chars().count()));
    }

    #[test]
    fn truncate_output_to_tokens_preserves_prefix_and_suffix() {
        let s = "abcdefghijklmnopqrstuvwxyz";
        let max_tokens = 10;
        let (truncated, original) = truncate_output_to_tokens(s, max_tokens);
        assert!(truncated.starts_with("abcde"));
        assert!(truncated.ends_with("vwxyz"));
        assert_eq!(original, Some(s.chars().count()));
    }

    #[test]
    fn format_exec_output_truncates_large_error() {
        let line = "very long execution error line that should trigger truncation\n";
        let large_error = line.repeat(2_500); // way beyond both byte and line limits

        let truncated = format_output_for_model_body(
            &large_error,
            MODEL_FORMAT_MAX_BYTES,
            MODEL_FORMAT_MAX_LINES,
        );

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
            body.len() <= MODEL_FORMAT_MAX_BYTES,
            "body exceeds byte limit: {} bytes",
            body.len()
        );
        assert_ne!(truncated, large_error);
    }

    #[test]
    fn format_exec_output_marks_byte_truncation_without_omitted_lines() {
        let long_line = "a".repeat(MODEL_FORMAT_MAX_BYTES + 50);
        let truncated = format_output_for_model_body(
            &long_line,
            MODEL_FORMAT_MAX_BYTES,
            MODEL_FORMAT_MAX_LINES,
        );

        assert_ne!(truncated, long_line);
        let marker_line =
            format!("[... output truncated to fit {MODEL_FORMAT_MAX_BYTES} bytes ...]");
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
            format_output_for_model_body(&content, MODEL_FORMAT_MAX_BYTES, MODEL_FORMAT_MAX_LINES),
            content
        );
    }

    #[test]
    fn format_exec_output_reports_omitted_lines_and_keeps_head_and_tail() {
        let total_lines = MODEL_FORMAT_MAX_LINES + 100;
        let content: String = (0..total_lines)
            .map(|idx| format!("line-{idx}\n"))
            .collect();

        let truncated =
            format_output_for_model_body(&content, MODEL_FORMAT_MAX_BYTES, MODEL_FORMAT_MAX_LINES);

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

        let truncated =
            format_output_for_model_body(&content, MODEL_FORMAT_MAX_BYTES, MODEL_FORMAT_MAX_LINES);

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
        // Arrange: several text items, none exceeding per-item limit, but total exceeds budget.
        let budget = MODEL_FORMAT_MAX_BYTES;
        let t1_len = (budget / 2).saturating_sub(10);
        let t2_len = (budget / 2).saturating_sub(10);
        let remaining_after_t1_t2 = budget.saturating_sub(t1_len + t2_len);
        let t3_len = 50; // gets truncated to remaining_after_t1_t2
        let t4_len = 5; // omitted
        let t5_len = 7; // omitted

        let t1 = "a".repeat(t1_len);
        let t2 = "b".repeat(t2_len);
        let t3 = "c".repeat(t3_len);
        let t4 = "d".repeat(t4_len);
        let t5 = "e".repeat(t5_len);

        let items = vec![
            FunctionCallOutputContentItem::InputText { text: t1 },
            FunctionCallOutputContentItem::InputText { text: t2 },
            FunctionCallOutputContentItem::InputImage {
                image_url: "img:mid".to_string(),
            },
            FunctionCallOutputContentItem::InputText { text: t3 },
            FunctionCallOutputContentItem::InputText { text: t4 },
            FunctionCallOutputContentItem::InputText { text: t5 },
        ];

        let output = globally_truncate_function_output_items(&items);

        // Expect: t1 (full), t2 (full), image, t3 (truncated), summary mentioning 2 omitted.
        assert_eq!(output.len(), 5);

        let first_text = match &output[0] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected first item: {other:?}"),
        };
        assert_eq!(first_text.len(), t1_len);

        let second_text = match &output[1] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected second item: {other:?}"),
        };
        assert_eq!(second_text.len(), t2_len);

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
        assert_eq!(fourth_text.len(), remaining_after_t1_t2);

        let summary_text = match &output[4] {
            FunctionCallOutputContentItem::InputText { text } => text,
            other => panic!("unexpected summary item: {other:?}"),
        };
        assert!(summary_text.contains("omitted 2 text items"));
    }
}
