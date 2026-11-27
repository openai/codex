//! Render source locations in the file reader using the transcript's wrapped rows.
//! Document chunks have no hidden entries, so scroll state needs only the visible rows.

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use super::*;

/// A document can be replaced on disk; retain the offset, not the old rendered cells.
#[derive(Clone, Copy)]
pub(crate) struct DocumentPosition(Position);

/// Resolved logical lines and the grapheme-aligned byte offset used for scrolling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DocumentTarget {
    pub(crate) first_line: usize,
    pub(crate) last_line: usize,
    pub(crate) offset: usize,
    pub(crate) mark_column: bool,
}

/// A terminal may not report its background. Pair the syntax theme's canvas with
/// its text color for unstyled glyphs, and preserve syntax-specific foregrounds.
fn source_highlight() -> (Style, Option<Color>) {
    use crate::render::highlight::convert_syntect_color;
    let (background, foreground) = match crate::terminal_palette::default_bg() {
        Some(bg) => (Some(bg), None),
        None => crate::render::highlight::with_syntax_theme(|theme| {
            let background = match theme.settings.background.and_then(convert_syntect_color) {
                Some(Color::Rgb(r, g, b)) => Some((r, g, b)),
                _ => None,
            };
            (
                background,
                theme.settings.foreground.and_then(convert_syntect_color),
            )
        }),
    };
    let style = crate::style::user_message_style_for(background);
    if matches!(style.bg, None | Some(Color::Reset)) {
        (crate::style::selection_style(), None)
    } else {
        (style, foreground)
    }
}

impl TranscriptView {
    /// During a live resize, wrap the already rendered file instead of parsing it at every width.
    pub(crate) fn set_document_resize_preview(&mut self, active: bool) {
        self.cache.set_document_resize_preview(active);
    }

    pub(crate) fn document_position(&self) -> DocumentPosition {
        DocumentPosition(self.position)
    }

    pub(crate) fn restore_document_position(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        position: DocumentPosition,
    ) {
        if let Position::Reading(anchor) = position.0
            && let Some(cell) = cells.get(anchor.index)
        {
            self.position = Position::Reading(Anchor {
                key: EntryKey::cell(cell),
                ..anchor
            });
        } else {
            self.jump_to_latest();
        }
    }

    pub(crate) fn render_document(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
        line_starts: &[usize],
        target: Option<DocumentTarget>,
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
        self.paint_rows(text, buf, cells, RenderContent::Document);
        self.finish_collapse_layout(cells);
        if line_starts.is_empty() {
            return;
        }
        let highlight = target.map(|target| (target, source_highlight()));
        for (y, visible) in (area.top()..area.bottom()).zip(&self.visible) {
            let offset = visible.layout.position_at(visible.row, /*column*/ 0);
            let line = line_starts.partition_point(|start| *start <= offset);
            if gutter > 0 && line > 0 && line_starts[line - 1] == offset {
                Line::from(line.to_string())
                    .dim()
                    .right_aligned()
                    .render(Rect::new(area.x, y, digits, /*height*/ 1), buf);
            }
            if let Some((target, (style, foreground))) = highlight
                && (target.first_line..=target.last_line).contains(&line)
            {
                for x in text.left()..text.right() {
                    let cell = &mut buf[(x, y)];
                    cell.set_style(style);
                    if let Some(fg) = foreground
                        && cell.fg == Color::Reset
                    {
                        cell.set_fg(fg);
                    }
                }
                if target.mark_column
                    && line == target.first_line
                    && visible.row == visible.layout.row_for_offset(target.offset)
                {
                    let x = text
                        .x
                        .saturating_add(visible.layout.column_for_offset(target.offset));
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
        &self,
        area: Rect,
        buf: &mut Buffer,
        cells: &[Arc<dyn HistoryCell>],
    ) {
        if area.height < 2 || area.width < 3 {
            return;
        }
        // The rendered rows already tell us whether more document content follows.
        // Measuring the final chunk here can evict the visible chunk on every redraw.
        let above = self
            .visible
            .first()
            .is_some_and(|row| (row.index, row.row) > (0, 0));
        let below = self.document_has_rows_below(cells);
        for (visible, row, arrow) in [(above, area.top(), "↑"), (below, area.bottom() - 1, "↓")]
        {
            if visible {
                buf[(area.right() - 1, row)]
                    .set_symbol(arrow)
                    .set_style(crate::style::secondary_text_style());
            }
        }
    }

    pub(super) fn document_has_rows_below(&self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        self.visible
            .last()
            .is_some_and(|row| row.row + 1 < row.layout.row_count() || row.index + 1 < cells.len())
    }
}
