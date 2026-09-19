//! Shared bounds for owned-transcript action previews; expansion uses retained source content.

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::text::Line;

pub(crate) const DETAIL_PREVIEW_LINES: usize = 3;

/// Clip a preview row without teaching selection to copy text that is currently hidden.
pub(crate) fn clipped_line(line: Line<'static>, width: u16) -> HyperlinkLine {
    truncate_line_with_ellipsis_if_overflow(line, usize::from(width)).into()
}
