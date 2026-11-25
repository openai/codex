//! Render source locations in the file reader using the transcript's wrapped rows.

use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use super::*;

/// A document can be replaced on disk; retain the offset, not the old rendered cells.
#[derive(Clone, Copy)]
pub(crate) enum DocumentPosition {
    Offset(usize),
    Latest,
}

impl TranscriptView {
    /// During a live resize, wrap the already rendered file instead of parsing it at every width.
    pub(crate) fn set_document_resize_preview(&mut self, active: bool) {
        self.cache.set_document_resize_preview(active);
    }

    pub(crate) fn document_position(&self) -> DocumentPosition {
        match self.position {
            Position::Reading(anchor) => DocumentPosition::Offset(anchor.offset),
            Position::Latest => DocumentPosition::Latest,
        }
    }

    pub(crate) fn render_document(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
        line_starts: &[usize],
        target: Option<(usize, usize)>,
    ) {
        let digits = line_starts
            .len()
            .checked_ilog10()
            .map_or(0, |n| n as u16 + 1);
        let gutter = if digits > 0 && area.width > 40 + digits {
            digits + 1
        } else {
            0
        };
        let text = Rect::new(area.x + gutter, area.y, area.width - gutter, area.height);
        self.render(text, buf, cells);
        if line_starts.is_empty() {
            return;
        }
        for (y, visible) in (area.top()..area.bottom()).zip(&self.visible) {
            let offset = visible.layout.position_at(visible.row, /*column*/ 0);
            let line = line_starts.partition_point(|start| *start <= offset);
            if gutter > 0 && line > 0 && line_starts[line - 1] == offset {
                Line::from(line.to_string())
                    .dim()
                    .right_aligned()
                    .render(Rect::new(area.x, y, digits, /*height*/ 1), buf);
            }
            if let Some((target_line, target_offset)) = target
                && line == target_line
            {
                let mut style = crate::style::user_message_style();
                if style.bg.is_none() {
                    style = crate::style::selection_style();
                }
                buf.set_style(Rect::new(text.x, y, text.width, /*height*/ 1), style);
                if visible.row == visible.layout.row_for_offset(target_offset) {
                    let x = text
                        .x
                        .saturating_add(visible.layout.column_for_offset(target_offset));
                    if x < text.right() {
                        let cell = &mut buf[(x, y)];
                        let style = cell.style();
                        cell.set_style(if style.add_modifier.contains(Modifier::REVERSED) {
                            style.not_reversed()
                        } else {
                            style.reversed()
                        });
                    }
                }
            }
        }
    }

    pub(crate) fn paint_document_scroll_indicators(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
    ) {
        if area.height < 2 || area.width < 3 {
            return;
        }
        let start = self.start(cells);
        for (visible, row, arrow) in [
            (start > (0, 0), area.top(), "↑"),
            (start < self.bottom_start(cells), area.bottom() - 1, "↓"),
        ] {
            if visible {
                buf[(area.right() - 1, row)]
                    .set_symbol(arrow)
                    .set_style(crate::style::secondary_text_style());
            }
        }
    }
}
