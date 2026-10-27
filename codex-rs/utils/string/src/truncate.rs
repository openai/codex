//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries.

use std::ops::Range;

const APPROX_BYTES_PER_TOKEN: usize = 4;

/// Replaces one UTF-8-aligned range with a notice. Callers can apply the same
/// edit to annotated text without losing the sources of the retained slices.
pub struct TextTruncation {
    pub removed: Range<usize>,
    pub marker: String,
}

impl TextTruncation {
    /// Applies this edit to the string it was planned from.
    pub fn apply(&self, original: &str) -> String {
        assemble_truncated_output(
            &original[..self.removed.start],
            &original[self.removed.end..],
            &self.marker,
        )
    }
}

/// Plans the existing head/tail token truncation. The marker is outside the
/// retained-text budget, just as it is in [`truncate_middle_with_token_budget`].
pub fn plan_token_truncation(s: &str, max_tokens: usize) -> Option<TextTruncation> {
    let max_bytes = approx_bytes_for_tokens(max_tokens);
    if s.len() <= max_bytes {
        return None;
    }
    let (left_budget, right_budget) = split_budget(max_bytes);
    let removed = s.floor_char_boundary(left_budget)..s.ceil_char_boundary(s.len() - right_budget);
    let removed_tokens = approx_tokens_from_byte_count(s.len() - max_bytes);
    Some(TextTruncation {
        removed,
        marker: format!("…{removed_tokens} tokens truncated…"),
    })
}

/// Retain at most `max_bytes` from a string's ends and add a character-count
/// marker outside that budget.
///
/// Use [`truncate_middle_with_marker`] when the complete result must fit a hard
/// byte limit.
pub fn truncate_middle_chars(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let (left_budget, right_budget) = split_budget(max_bytes);
    let (removed_chars, left, right) = split_string(s, left_budget, right_budget);
    assemble_truncated_output(left, right, &format!("…{removed_chars} chars truncated…"))
}

/// Truncate the middle of a UTF-8 string so the result, including `marker`, is
/// at most `max_bytes` bytes.
///
/// Returns the original string when it already fits. If the marker itself does
/// not fit, returns the longest original prefix that does.
pub fn truncate_middle_with_marker(s: &str, max_bytes: usize, marker: &str) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }

    let Some(content_budget) = max_bytes.checked_sub(marker.len()) else {
        return s[..s.floor_char_boundary(max_bytes)].to_string();
    };
    let (left_budget, right_budget) = split_budget(content_budget);
    let prefix_end = s.floor_char_boundary(left_budget);
    let suffix_start = s.ceil_char_boundary(s.len().saturating_sub(right_budget));

    assemble_truncated_output(&s[..prefix_end], &s[suffix_start..], marker)
}

/// Truncate the middle of a UTF-8 string to at most `max_tokens` approximate
/// tokens, preserving the beginning and the end. Returns the possibly
/// truncated string and `Some(original_token_count)` if truncation occurred;
/// otherwise returns the original string and `None`.
pub fn truncate_middle_with_token_budget(s: &str, max_tokens: usize) -> (String, Option<u64>) {
    let Some(truncation) = plan_token_truncation(s, max_tokens) else {
        return (s.to_string(), None);
    };
    let truncated = truncation.apply(s);
    let total_tokens = u64::try_from(approx_token_count(s)).unwrap_or(u64::MAX);

    if truncated == s {
        (truncated, None)
    } else {
        (truncated, Some(total_tokens))
    }
}

pub fn approx_token_count(text: &str) -> usize {
    let len = text.len();
    len.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

pub const fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens.saturating_mul(APPROX_BYTES_PER_TOKEN)
}

pub fn approx_tokens_from_byte_count(bytes: usize) -> u64 {
    let bytes_u64 = bytes as u64;
    bytes_u64.saturating_add((APPROX_BYTES_PER_TOKEN as u64).saturating_sub(1))
        / (APPROX_BYTES_PER_TOKEN as u64)
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

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn assemble_truncated_output(prefix: &str, suffix: &str, marker: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + marker.len() + suffix.len() + 1);
    out.push_str(prefix);
    out.push_str(marker);
    out.push_str(suffix);
    out
}

#[cfg(test)]
mod tests;
