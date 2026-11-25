//! Preserve complete web destinations on visible, independently wrapped textarea rows.

use super::text_for_display;
use super::wrapping::visible_prefix;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::mark_buffer_hyperlinks;
use crate::terminal_hyperlinks::source_text_width;
use crate::terminal_hyperlinks::web_links_in_text;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::ops::Range;

/// Destinations and row offsets retained for one lazily initialized textarea wrap state.
#[derive(Clone, Debug, Default)]
pub(super) struct HyperlinkCache {
    hyperlinks: Vec<TerminalHyperlink>,
    source_columns: Vec<usize>,
}

impl HyperlinkCache {
    /// Scan the draft once and precompute row offsets for constant-time scrolled lookup.
    pub(super) fn new(text: &str, lines: &[Range<usize>]) -> Self {
        let display_text = text_for_display(text);
        let hyperlinks = web_links_in_text(display_text.as_ref());
        if hyperlinks.is_empty() {
            return Self::default();
        }

        let mut source_column = 0;
        let mut previous_start = 0;
        let source_columns = lines
            .iter()
            .map(|line| {
                source_column += source_text_width(&display_text[previous_start..line.start]);
                previous_start = line.start;
                source_column
            })
            .collect();

        Self {
            hyperlinks,
            source_columns,
        }
    }

    /// Attach complete destinations after styling without revisiting offscreen draft text.
    pub(super) fn mark(
        &self,
        buf: &mut Buffer,
        area: Rect,
        text: &str,
        lines: &[Range<usize>],
        visible_rows: Range<usize>,
    ) {
        if area.width == 0 || area.height == 0 || self.hyperlinks.is_empty() {
            return;
        }

        for (row, index) in visible_rows.enumerate() {
            let line_range = &lines[index];
            let source_column = self.source_columns[index];
            let raw_visible = visible_prefix(
                &text[line_range.start..line_range.end.saturating_sub(1)],
                area.width,
            );
            let visible = text_for_display(raw_visible);
            let line_end = source_column + source_text_width(visible.as_ref());
            let first_hyperlink = self
                .hyperlinks
                .partition_point(|hyperlink| hyperlink.columns.end <= source_column);

            let mut annotated = HyperlinkLine::new(Line::from(visible.into_owned()));
            for hyperlink in self.hyperlinks[first_hyperlink..]
                .iter()
                .take_while(|hyperlink| hyperlink.columns.start < line_end)
            {
                let start = hyperlink.columns.start.max(source_column) - source_column;
                let end = hyperlink.columns.end.min(line_end) - source_column;
                annotated.hyperlinks.push(TerminalHyperlink::web(
                    start..end,
                    hyperlink.destination.clone(),
                ));
            }

            if !annotated.hyperlinks.is_empty() {
                let row_area =
                    Rect::new(area.x, area.y + row as u16, area.width, /*height*/ 1);
                mark_buffer_hyperlinks(buf, row_area, &[annotated], /*scroll_rows*/ 0);
            }
        }
    }
}

#[cfg(test)]
#[path = "hyperlinks_tests.rs"]
mod tests;
