//! Guardian's shared UTF-8-safe, prefix/suffix text truncation and observations.
//!
//! The existing XML omission marker is preserved, including returning the whole
//! marker when a token budget is too small to contain it. Content fitting measures
//! the rendered entry so JSON escaping and record metadata count toward its limit.

use crate::budget::section_content_tokens;
use crate::composition::SectionContent;
use codex_protocol::protocol::TruncationPolicy;

const BINARY_SEARCH_DIVISOR: usize = 2;

/// Shortens only the text body, keeping record metadata and both ends intact.
/// Falls back to `min_text_tokens` when the limit cannot be met. Returns whether
/// the result fits; restores the original body if shortening saves no tokens.
pub(crate) fn truncate_content_to_fit(
    content: &mut SectionContent,
    max_tokens: usize,
    min_text_tokens: usize,
) -> bool {
    let original_tokens = section_content_tokens(content);
    let Some(original_body) = content.replace_text(String::new()) else {
        return false;
    };
    let mut upper = TruncationPolicy::Bytes(original_body.len()).token_budget();
    let mut lower = min_text_tokens.min(upper);
    let mut selected = lower;
    while lower < upper {
        let mid = lower + (upper - lower).div_ceil(BINARY_SEARCH_DIVISOR);
        let _ = content.replace_text(truncate_text(&original_body, mid));
        if section_content_tokens(content) <= max_tokens {
            lower = mid;
            selected = mid;
        } else {
            upper = mid - 1;
        }
    }
    let _ = content.replace_text(truncate_text(&original_body, selected));
    if section_content_tokens(content) >= original_tokens {
        let _ = content.replace_text(original_body);
    }
    section_content_tokens(content) <= max_tokens
}

/// Truncates text using Guardian's approximate token budget and omission marker.
///
/// Retains both ends on UTF-8 boundaries. Budgets smaller than the marker still
/// return the marker, so callers should reserve room for that fixed overhead.
pub fn truncate_text(text: &str, max_tokens: usize) -> String {
    let max_bytes = TruncationPolicy::Tokens(max_tokens).byte_budget();
    if text.len() <= max_bytes {
        return text.to_owned();
    }

    let omitted_tokens =
        TruncationPolicy::Bytes(text.len().saturating_sub(max_bytes)).token_budget();
    let marker = format!("<truncated omitted_approx_tokens=\"{omitted_tokens}\" />");
    if max_bytes <= marker.len() {
        return marker;
    }

    let available_bytes = max_bytes - marker.len();
    let prefix_bytes = available_bytes / 2;
    let suffix_bytes = available_bytes - prefix_bytes;
    let mut prefix_end = prefix_bytes;
    while !text.is_char_boundary(prefix_end) {
        prefix_end -= 1;
    }
    let mut suffix_start = text.len() - suffix_bytes;
    while !text.is_char_boundary(suffix_start) {
        suffix_start += 1;
    }

    format!("{}{marker}{}", &text[..prefix_end], &text[suffix_start..])
}

#[cfg(test)]
#[path = "truncation_tests.rs"]
mod tests;

/// Actual evidence reduction, reported by consumers using their existing metrics.
#[derive(Clone)]
pub struct TruncationObservation {
    pub component: &'static str,
    pub original_bytes: usize,
    pub retained_bytes: usize,
}
