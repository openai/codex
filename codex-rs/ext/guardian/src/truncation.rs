use codex_utils_output_truncation::approx_bytes_for_tokens;
use codex_utils_output_truncation::approx_tokens_from_byte_count;

const TRUNCATION_TAG: &str = "truncated";

/// Truncates text to an approximate token cap while retaining its prefix and suffix.
pub fn guardian_truncate_text(content: &str, token_cap: usize) -> (String, bool) {
    if content.is_empty() {
        return (String::new(), false);
    }

    let max_bytes = approx_bytes_for_tokens(token_cap);
    if content.len() <= max_bytes {
        return (content.to_string(), false);
    }

    let omitted_tokens = approx_tokens_from_byte_count(content.len().saturating_sub(max_bytes));
    let marker = format!("<{TRUNCATION_TAG} omitted_approx_tokens=\"{omitted_tokens}\" />");
    if max_bytes <= marker.len() {
        return (marker, true);
    }

    let available_bytes = max_bytes.saturating_sub(marker.len());
    let prefix_budget = available_bytes / 2;
    let suffix_budget = available_bytes.saturating_sub(prefix_budget);
    let (prefix, suffix) = split_guardian_truncation_bounds(content, prefix_budget, suffix_budget);

    (format!("{prefix}{marker}{suffix}"), true)
}

fn split_guardian_truncation_bounds(
    content: &str,
    prefix_bytes: usize,
    suffix_bytes: usize,
) -> (&str, &str) {
    if content.is_empty() {
        return ("", "");
    }

    let len = content.len();
    let suffix_start_target = len.saturating_sub(suffix_bytes);
    let mut prefix_end = 0usize;
    let mut suffix_start = len;
    let mut suffix_started = false;

    for (index, ch) in content.char_indices() {
        let char_end = index + ch.len_utf8();
        if char_end <= prefix_bytes {
            prefix_end = char_end;
            continue;
        }

        if index >= suffix_start_target {
            if !suffix_started {
                suffix_start = index;
                suffix_started = true;
            }
            continue;
        }
    }

    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    (&content[..prefix_end], &content[suffix_start..])
}
