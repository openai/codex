//! Retain renderer output and row offsets, preserving Ratatui wrapping for overflow lines.

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use crate::width::display_width;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellWidth;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct TextLayout {
    pub(super) rows: Vec<TextRow>,
    pub(super) byte_len: usize,
}

#[derive(Clone)]
pub(super) struct TextRow {
    pub(super) line: Arc<HyperlinkLine>,
    pub(super) offset: u16,
}

impl TextLayout {
    pub(super) fn new(lines: Vec<HyperlinkLine>, width: u16) -> Self {
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

    pub(super) fn with_leading_separator(mut self) -> Self {
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

    pub(super) fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer, row: usize) {
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

    /// Highlight the entry's text while leaving synthetic gutters and padding untouched.
    pub(super) fn highlight(&self, area: Rect, buf: &mut Buffer, row: usize) {
        let row = &self.rows[row];
        let line = &row.line;
        let text = line.line.to_string();
        let prefix = if row.offset == 0 {
            line.source.as_ref().map_or(
                /*default*/ 0,
                |source| display_width(&text[..source.prefix_bytes]),
            )
        } else {
            0
        };
        let width = display_width(&text);
        let columns = usize::from(area.width);
        let alignment = match line.line.alignment.unwrap_or(Alignment::Left) {
            Alignment::Left => 0,
            Alignment::Center => (columns / 2).saturating_sub(width / 2),
            Alignment::Right => columns.saturating_sub(width),
        };
        let (alignment, end) = if width > columns {
            let end = (0..area.width)
                .rfind(|x| !buf[(area.x + x, area.y)].symbol().trim().is_empty())
                .map_or(/*default*/ 0, |x| {
                    usize::from(x + buf[(area.x + x, area.y)].cell_width())
                });
            let start = if line.line.alignment.unwrap_or(Alignment::Left) == Alignment::Left {
                0
            } else {
                (0..area.width)
                    .find(|x| !buf[(area.x + x, area.y)].symbol().trim().is_empty())
                    .map_or(/*default*/ 0, usize::from)
            };
            (start, end)
        } else {
            (alignment, alignment + width)
        };
        for column in alignment + prefix..end.min(columns) {
            buf[(area.x + column as u16, area.y)]
                .set_style(Style::default().add_modifier(Modifier::REVERSED));
        }
    }
}
