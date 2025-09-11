//! Utilities for truncating large chunks of output while preserving a prefix
//! and suffix on UTF-8 boundaries.

/// Truncate the middle of a UTF-8 string to at most `max_bytes` bytes,
/// preserving the beginning and the end. Returns the possibly truncated
/// string and `Some(original_token_count)` (estimated at 4 bytes/token)
/// if truncation occurred; otherwise returns the original string and `None`.
pub(crate) fn truncate_middle(s: &str, max_bytes: usize) -> (String, Option<u64>) {
    if s.len() <= max_bytes {
        return (s.to_string(), None);
    }

    let est_tokens = (s.len() as u64).div_ceil(4);
    if max_bytes == 0 {
        return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
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

    let mut guess_tokens = est_tokens;
    for _ in 0..4 {
        let marker = format!("…{guess_tokens} tokens truncated…");
        let marker_len = marker.len();
        let keep_budget = max_bytes.saturating_sub(marker_len);
        if keep_budget == 0 {
            return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
        }

        let left_budget = keep_budget / 2;
        let right_budget = keep_budget - left_budget;
        let prefix_end = pick_prefix_end(s, left_budget);
        let mut suffix_start = pick_suffix_start(s, right_budget);
        if suffix_start < prefix_end {
            suffix_start = prefix_end;
        }

        let kept_content_bytes = prefix_end + (s.len() - suffix_start);
        let truncated_content_bytes = s.len().saturating_sub(kept_content_bytes);
        let new_tokens = (truncated_content_bytes as u64).div_ceil(4);

        if new_tokens == guess_tokens {
            let mut out = String::with_capacity(marker_len + kept_content_bytes + 1);
            out.push_str(&s[..prefix_end]);
            out.push_str(&marker);
            out.push('\n');
            out.push_str(&s[suffix_start..]);
            return (out, Some(est_tokens));
        }

        guess_tokens = new_tokens;
    }

    let marker = format!("…{guess_tokens} tokens truncated…");
    let marker_len = marker.len();
    let keep_budget = max_bytes.saturating_sub(marker_len);
    if keep_budget == 0 {
        return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
    }

    let left_budget = keep_budget / 2;
    let right_budget = keep_budget - left_budget;
    let prefix_end = pick_prefix_end(s, left_budget);
    let suffix_start = pick_suffix_start(s, right_budget);

    let mut out = String::with_capacity(marker_len + prefix_end + (s.len() - suffix_start) + 1);
    out.push_str(&s[..prefix_end]);
    out.push_str(&marker);
    out.push('\n');
    out.push_str(&s[suffix_start..]);
    (out, Some(est_tokens))
}

#[cfg(test)]
mod tests {
    use super::truncate_middle;

    #[test]
    fn truncate_middle_no_newlines_fallback() {
        let s = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ*";
        let max_bytes = 32;
        let (out, original) = truncate_middle(s, max_bytes);
        assert!(out.starts_with("abc"));
        assert!(out.contains("tokens truncated"));
        assert!(out.ends_with("XYZ*"));
        assert_eq!(original, Some((s.len() as u64).div_ceil(4)));
    }

    #[test]
    fn truncate_middle_prefers_newline_boundaries() {
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
        assert_eq!(tokens, Some(20));
    }

    #[test]
    fn truncate_middle_handles_utf8_content() {
        let s = "😀😀😀😀😀😀😀😀😀😀\nsecond line with ascii text\n";
        let max_bytes = 32;
        let (out, tokens) = truncate_middle(s, max_bytes);

        assert!(out.contains("tokens truncated"));
        assert!(!out.contains('\u{fffd}'));
        assert_eq!(tokens, Some((s.len() as u64).div_ceil(4)));
    }

    #[test]
    fn truncate_middle_prefers_newline_boundaries_2() {
        // Build a multi-line string of 20 numbered lines (each "NNN\n").
        let mut s = String::new();
        for i in 1..=20 {
            s.push_str(&format!("{i:03}\n"));
        }
        // Total length: 20 lines * 4 bytes per line = 80 bytes.
        assert_eq!(s.len(), 80);

        // Choose a cap that forces truncation while leaving room for
        // a few lines on each side after accounting for the marker.
        let max_bytes = 64;
        // Expect exact output: first 4 lines, marker, last 4 lines, and correct token estimate (80/4 = 20).
        assert_eq!(
            truncate_middle(&s, max_bytes),
            (
                r#"001
002
003
004
…12 tokens truncated…
017
018
019
020
"#
                .to_string(),
                Some(20)
            )
        );
    }
}
