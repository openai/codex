use ratatui::text::Line;

/// Convenience: compute the highlight range for the Nth last user message.
pub(crate) fn highlight_range_for_nth_last_user(
    lines: &[Line<'_>],
    n: usize,
) -> Option<(usize, usize)> {
    let header = find_nth_last_user_header_index(lines, n)?;
    Some(highlight_range_from_header(lines, header))
}

/// Compute the wrapped display-line offset before `header_idx`, for a given width.
pub(crate) fn wrapped_offset_before(lines: &[Line<'_>], header_idx: usize, width: u16) -> usize {
    let before = &lines[0..header_idx];
    crate::insert_history::word_wrap_lines(before, width).len()
}

/// Find the header index for the Nth last user message in the transcript.
/// Returns `None` if `n == 0` or there are fewer than `n` user messages.
pub(crate) fn find_nth_last_user_header_index(lines: &[Line<'_>], n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let mut found = 0usize;
    for (idx, line) in lines.iter().enumerate().rev() {
        let content: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        if content.trim() == "user" {
            found += 1;
            if found == n {
                return Some(idx);
            }
        }
    }
    None
}

/// Extract the text content of the Nth last user message.
/// The message body is considered to be the lines following the "user" header
/// until the first blank line.
pub(crate) fn nth_last_user_text(lines: &[Line<'_>], n: usize) -> Option<String> {
    let header_idx = find_nth_last_user_header_index(lines, n)?;
    extract_message_text_after_header(lines, header_idx)
}

/// Extract message text starting after `header_idx` until the first blank line.
fn extract_message_text_after_header(lines: &[Line<'_>], header_idx: usize) -> Option<String> {
    let start = header_idx + 1;
    let mut out: Vec<String> = Vec::new();
    for line in lines.iter().skip(start) {
        let is_blank = line
            .spans
            .iter()
            .all(|s| s.content.as_ref().trim().is_empty());
        if is_blank {
            break;
        }
        let text: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        out.push(text);
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

/// Given a header index, return the inclusive range for the message block
/// [header_idx, end) where end is the first blank line after the header or the
/// end of the transcript.
fn highlight_range_from_header(lines: &[Line<'_>], header_idx: usize) -> (usize, usize) {
    let mut end = header_idx + 1;
    while end < lines.len() {
        let is_blank = lines[end]
            .spans
            .iter()
            .all(|s| s.content.as_ref().trim().is_empty());
        if is_blank {
            break;
        }
        end += 1;
    }
    (header_idx, end)
}
