//! Retain renderer output and row offsets, preserving Ratatui wrapping for overflow lines.

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct TextLayout {
    pub(crate) rows: Vec<TextRow>,
    pub(crate) byte_len: usize,
}

#[derive(Clone)]
pub(crate) struct TextRow {
    pub(crate) line: Arc<HyperlinkLine>,
    pub(crate) offset: u16,
}

impl TextLayout {
    pub(crate) fn new(lines: Vec<HyperlinkLine>, width: u16) -> Self {
        let byte_len = lines
            .iter()
            .flat_map(|line| &line.line.spans)
            .map(|span| span.content.len())
            .sum();
        let rows = lines
            .into_iter()
            .flat_map(|line| {
                // Most history renderers already wrap to width. Use the original paragraph for
                // overflow so indentation, hyphens, and wide glyphs follow the same rules as paint.
                let count = if crate::line_truncation::line_width(&line.line)
                    <= usize::from(width.max(/*other*/ 1))
                {
                    1
                } else {
                    HyperlinkParagraph::new(std::slice::from_ref(&line), Style::default())
                        .line_count(width.max(/*other*/ 1))
                };
                let line = Arc::new(line);
                // Ratatui's paragraph scroll offset is u16, as in the existing pager.
                (0..u16::try_from(count).unwrap_or(u16::MAX)).map(move |offset| TextRow {
                    line: Arc::clone(&line),
                    offset,
                })
            })
            .collect();
        Self { rows, byte_len }
    }

    pub(crate) fn with_leading_separator(mut self) -> Self {
        if !self.rows.is_empty() {
            self.rows.insert(
                /*index*/ 0,
                TextRow {
                    line: Arc::new(HyperlinkLine::from("")),
                    offset: 0,
                },
            );
        }
        self
    }

    pub(crate) fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer, row: usize) {
        let start = row.min(self.rows.len());
        let end = row
            .saturating_add(usize::from(area.height))
            .min(self.rows.len());
        let mut y = area.y;
        for rows in self.rows[start..end].chunk_by(|a, b| Arc::ptr_eq(&a.line, &b.line)) {
            let first = &rows[0];
            let height = rows.len() as u16;
            HyperlinkParagraph::new(std::slice::from_ref(first.line.as_ref()), Style::default())
                .scroll(first.offset)
                .render(Rect::new(area.x, y, area.width, height), buf);
            y += height;
        }
    }
}
