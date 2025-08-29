use ratatui::text::Line;

/// Convenience: compute the highlight range for the Nth last user message.
pub(crate) fn highlight_range_for_nth_last_user(
    user_spans: &[(usize, usize)],
    n: usize,
) -> Option<(usize, usize)> {
    if n == 0 {
        return None;
    }
    let idx = user_spans.len().checked_sub(n)?;
    user_spans.get(idx).copied()
}

/// Compute the wrapped display-line offset before `header_idx`, for a given width.
pub(crate) fn wrapped_offset_before(lines: &[Line<'_>], header_idx: usize, width: u16) -> usize {
    let before = &lines[0..header_idx];
    crate::wrapping::word_wrap_lines(before, width as usize).len()
}

/// Find the header index for the Nth last user message in the transcript.
/// Returns `None` if `n == 0` or there are fewer than `n` user messages.
pub(crate) fn find_nth_last_user_header_index(
    user_spans: &[(usize, usize)],
    n: usize,
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let idx = user_spans.len().checked_sub(n)?;
    user_spans.get(idx).map(|(h, _)| *h)
}

/// Normalize a requested backtrack step `n` against the available user messages.
/// - Returns `0` if there are no user messages.
/// - Returns `n` if the Nth last user message exists.
/// - Otherwise wraps to `1` (the most recent user message).
pub(crate) fn normalize_backtrack_n(user_spans: &[(usize, usize)], n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if user_spans.len() >= n {
        return n;
    }
    if user_spans.is_empty() { 0 } else { 1 }
}

/// Extract the text content of the Nth last user message.
/// The message body is considered to be the lines following the "user" header
/// until the first blank line.
pub(crate) fn nth_last_user_text(
    lines: &[Line<'_>],
    user_spans: &[(usize, usize)],
    n: usize,
) -> Option<String> {
    if n == 0 {
        return None;
    }
    let idx = user_spans.len().checked_sub(n)?;
    let (header, end) = user_spans.get(idx).copied()?;
    let start = header.saturating_add(1);
    if start >= end || start >= lines.len() {
        return None;
    }
    let end = end.min(lines.len());
    let out: Vec<String> = lines[start..end]
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect();
    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str) -> Line<'static> {
        s.to_string().into()
    }

    fn transcript_with_users(count: usize) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
        // Build transcript lines and user spans as [header..end) where end excludes the last body line.
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for i in 0..count {
            // Simulate the structure produced by UserHistoryCell: blank, header, body
            lines.push(line(""));
            let header = lines.len();
            lines.push(line("user"));
            lines.push(line(&format!("message {i}")));
            let end = header + 1 + 1; // header + 1 (body begins) + body_len(1)
            spans.push((header, end));
        }
        (lines, spans)
    }

    #[test]
    fn normalize_wraps_to_one_when_past_oldest() {
        let (_, spans) = transcript_with_users(2);
        assert_eq!(normalize_backtrack_n(&spans, 1), 1);
        assert_eq!(normalize_backtrack_n(&spans, 2), 2);
        // Requesting 3rd when only 2 exist wraps to 1
        assert_eq!(normalize_backtrack_n(&spans, 3), 1);
    }

    #[test]
    fn normalize_returns_zero_when_no_user_messages() {
        let (_, spans) = transcript_with_users(0);
        assert_eq!(normalize_backtrack_n(&spans, 1), 0);
        assert_eq!(normalize_backtrack_n(&spans, 5), 0);
    }

    #[test]
    fn normalize_keeps_valid_n() {
        let (_, spans) = transcript_with_users(3);
        assert_eq!(normalize_backtrack_n(&spans, 2), 2);
    }
}
