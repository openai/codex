//! Keep the prompt associated with the first visible answer in a presentation-only top row.
//! The header is never transcript content, so copying, searching and exporting stay exact.
//! A header that yields at a turn boundary stays hidden while that viewport and top row are held.

use super::*;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::sanitize_user_text;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;

/// Remember only a header canceled at a turn boundary, not a missing preceding prompt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SuppressedHeader {
    pub(super) viewport: Rect,
    pub(super) key: EntryKey,
    pub(super) row: usize,
}

pub(super) fn line(
    cells: &[Arc<dyn HistoryCell>],
    first: usize,
    width: u16,
) -> Option<Line<'static>> {
    // A prompt that is still visible supplies its own context. Only pin a preceding prompt.
    if width < 16
        || cells
            .get(first)
            .and_then(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
            .is_some_and(UserHistoryCell::has_visible_content)
    {
        return None;
    }
    let prompt = cells[..first.min(cells.len())]
        .iter()
        .rev()
        .filter_map(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
        .find(|prompt| prompt.has_visible_content())?;
    // Bound work even for pasted multi-megabyte prompts; sanitize before painting controls.
    let prefix: String = prompt.message.chars().take(/*n*/ 512).collect();
    let message = sanitize_user_text(prefix.into())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let message = if message.is_empty() {
        "[attachments]".to_owned()
    } else {
        message
    };
    Some(truncate_line_with_ellipsis_if_overflow(
        Line::from(message).style(crate::style::history_prompt_style()),
        usize::from(width),
    ))
}

#[cfg(test)]
#[path = "prompt_header_tests.rs"]
mod tests;
