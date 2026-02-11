pub(super) fn compact_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn truncate_text_for_storage(input: &str, max_bytes: usize, marker: &str) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }

    let budget_without_marker = max_bytes.saturating_sub(marker.len());
    let head_budget = budget_without_marker / 2;
    let tail_budget = budget_without_marker.saturating_sub(head_budget);
    let head = prefix_at_char_boundary(input, head_budget);
    let tail = suffix_at_char_boundary(input, tail_budget);

    format!("{head}{marker}{tail}")
}

pub(super) fn prefix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if max_bytes >= input.len() {
        return input;
    }

    let mut end = 0;
    for (idx, _) in input.char_indices() {
        if idx > max_bytes {
            break;
        }
        end = idx;
    }

    &input[..end]
}

pub(super) fn suffix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if max_bytes >= input.len() {
        return input;
    }

    let start_limit = input.len().saturating_sub(max_bytes);
    let mut start = input.len();
    for (idx, _) in input.char_indices().rev() {
        if idx < start_limit {
            break;
        }
        start = idx;
    }

    &input[start..]
}
