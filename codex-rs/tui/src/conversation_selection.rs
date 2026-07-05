//! Source-backed selection for the retained conversation viewport.
//!
//! Mouse coordinates are layout data, not clipboard data. A projection maps rendered terminal
//! cells back to ranges in canonical semantic text; copied output is always sliced from that text.

use std::ops::Range;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectionSegment {
    pub(crate) columns: Range<u16>,
    pub(crate) bytes: Range<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SelectionRow {
    pub(crate) segments: Vec<SelectionSegment>,
}

/// Canonical copy text plus a per-rendered-row mapping back into that text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CellSelectionProjection {
    text: Arc<str>,
    rows: Vec<SelectionRow>,
    separator_before: &'static str,
}

#[derive(Clone, Debug)]
struct SemanticGrapheme<'a> {
    text: &'a str,
    bytes: Range<usize>,
}

impl CellSelectionProjection {
    /// Build a projection by aligning the cell's rendered graphemes with semantic source text.
    ///
    /// `first_row_prefix_columns` marks presentation-only chrome prepended to every logical line.
    /// It applies only to the first terminal row of a logical line because Ratatui may perform one
    /// final character wrap for an overlong token.
    pub(crate) fn from_display_lines(
        text: String,
        lines: Vec<Line<'static>>,
        width: u16,
        first_row_prefix_columns: u16,
    ) -> Option<Self> {
        if text.is_empty() || width == 0 || lines.is_empty() {
            return None;
        }

        let semantic = semantic_graphemes(&text);
        let mut semantic_cursor = 0;
        let mut rows = Vec::new();

        for line in lines {
            let paragraph = Paragraph::new(line).wrap(Wrap { trim: false });
            let height = paragraph
                .line_count(width)
                .max(1)
                .try_into()
                .unwrap_or(u16::MAX);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            const UNTOUCHED: &str = "\0";
            let mut buffer = Buffer::filled(area, Cell::new(UNTOUCHED));
            paragraph.render(area, &mut buffer);

            for row_index in 0..height {
                let mut row = SelectionRow::default();
                let mut column = 0;
                while column < width {
                    let cell = &buffer[(column, row_index)];
                    let symbol = cell.symbol();
                    if cell.skip || symbol == UNTOUCHED {
                        column = column.saturating_add(1);
                        continue;
                    }
                    if symbol.is_empty() {
                        column = column.saturating_add(1);
                        continue;
                    }
                    let symbol_width = symbol.width().max(1).min(usize::from(u16::MAX)) as u16;
                    let columns = column..column.saturating_add(symbol_width).min(width);
                    if row_index == 0 && columns.start < first_row_prefix_columns {
                        column = column.saturating_add(symbol_width);
                        continue;
                    }

                    let Some(source_index) =
                        matching_semantic_index(&semantic, semantic_cursor, symbol)
                    else {
                        column = column.saturating_add(symbol_width);
                        continue;
                    };
                    let source = &semantic[source_index];
                    row.segments.push(SelectionSegment {
                        columns,
                        bytes: source.bytes.clone(),
                    });
                    semantic_cursor = source_index.saturating_add(1);
                    column = column.saturating_add(symbol_width);
                }
                rows.push(row);
            }
        }

        if rows.iter().all(|row| row.segments.is_empty()) {
            return None;
        }

        Some(Self {
            text: Arc::from(text),
            rows,
            separator_before: "\n\n",
        })
    }

    pub(crate) fn with_separator_before(mut self, separator_before: &'static str) -> Self {
        self.separator_before = separator_before;
        self
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn rows(&self) -> &[SelectionRow] {
        &self.rows
    }

    pub(crate) fn hit(&self, row: usize, column: u16) -> Option<Range<usize>> {
        self.rows.get(row)?.segments.iter().find_map(|segment| {
            segment
                .columns
                .contains(&column)
                .then(|| segment.bytes.clone())
        })
    }

    pub(crate) fn closest_hit(&self, row: usize, column: u16) -> Option<Range<usize>> {
        if let Some(hit) = self.hit(row, column) {
            return Some(hit);
        }
        let segments = &self.rows.get(row)?.segments;
        let first = segments.first()?;
        if column < first.columns.start {
            return Some(first.bytes.start..first.bytes.start);
        }
        let last = segments.last()?;
        if column >= last.columns.end {
            return Some(last.bytes.end..last.bytes.end);
        }

        segments.windows(2).find_map(|pair| {
            let [left, right] = pair else {
                return None;
            };
            if column < left.columns.end || column >= right.columns.start {
                return None;
            }
            let left_distance = column.saturating_sub(left.columns.end);
            let right_distance = right.columns.start.saturating_sub(column);
            if left_distance < right_distance {
                Some(left.bytes.end..left.bytes.end)
            } else {
                Some(right.bytes.start..right.bytes.start)
            }
        })
    }

    pub(crate) fn closest_hit_in_any_row(&self, row: usize, column: u16) -> Option<Range<usize>> {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(candidate_row, selection_row)| {
                let first = selection_row.segments.first()?;
                let last = selection_row.segments.last()?;
                let column_distance = if column < first.columns.start {
                    first.columns.start - column
                } else if column >= last.columns.end {
                    column.saturating_sub(last.columns.end.saturating_sub(1))
                } else {
                    0
                };
                Some((
                    (candidate_row.abs_diff(row), column_distance),
                    candidate_row,
                ))
            })
            .min_by_key(|(distance, _)| *distance)
            .and_then(|(_, candidate_row)| self.closest_hit(candidate_row, column))
    }
}

fn semantic_graphemes(text: &str) -> Vec<SemanticGrapheme<'_>> {
    text.grapheme_indices(/*is_extended*/ true)
        .map(|(start, grapheme)| SemanticGrapheme {
            text: grapheme,
            bytes: start..start + grapheme.len(),
        })
        .collect()
}

fn matching_semantic_index(
    semantic: &[SemanticGrapheme<'_>],
    cursor: usize,
    rendered: &str,
) -> Option<usize> {
    let current = semantic.get(cursor)?;
    if current.text == rendered {
        return Some(cursor);
    }
    if rendered.chars().all(char::is_whitespace) {
        return current
            .text
            .chars()
            .all(|character| character.is_whitespace() && character != '\n' && character != '\r')
            .then_some(cursor);
    }

    for (index, grapheme) in semantic.iter().enumerate().skip(cursor) {
        if grapheme.text == rendered {
            return Some(index);
        }
        if index != cursor && !grapheme.text.chars().all(char::is_whitespace) {
            break;
        }
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectionPoint {
    pub(crate) cell: usize,
    pub(crate) bytes: Range<usize>,
}

#[derive(Default)]
pub(crate) struct ConversationSelection {
    anchor: Option<SelectionPoint>,
    focus: Option<SelectionPoint>,
    dragged: bool,
}

impl ConversationSelection {
    pub(crate) fn start(&mut self, point: SelectionPoint) {
        self.anchor = Some(point.clone());
        self.focus = Some(point);
        self.dragged = false;
    }

    pub(crate) fn update(&mut self, point: SelectionPoint) -> bool {
        if self.anchor.is_none() {
            return false;
        }
        self.focus = Some(point);
        self.dragged = true;
        true
    }

    pub(crate) fn finish(
        &mut self,
        point: Option<SelectionPoint>,
        projections: &[Option<CellSelectionProjection>],
    ) -> Option<String> {
        self.set_release_point(point);
        let text = self
            .dragged
            .then(|| self.selected_text(projections))
            .flatten();
        self.cancel();
        text.filter(|text| !text.is_empty())
    }

    pub(crate) fn set_release_point(&mut self, point: Option<SelectionPoint>) {
        if let Some(point) = point {
            self.focus = Some(point);
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.dragged = false;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.anchor.is_some()
    }

    pub(crate) fn selected_cell_span(&self) -> Option<Range<usize>> {
        let (start, end) = self.ordered_points()?;
        Some(start.cell..end.cell.saturating_add(1))
    }

    pub(crate) fn selected_bytes_for_cell(
        &self,
        cell: usize,
        text_len: usize,
    ) -> Option<Range<usize>> {
        let (start, end) = self.ordered_points()?;
        if cell < start.cell || cell > end.cell {
            return None;
        }
        let range = if start.cell == end.cell {
            start.bytes.start..end.bytes.end
        } else if cell == start.cell {
            start.bytes.start..text_len
        } else if cell == end.cell {
            0..end.bytes.end
        } else {
            0..text_len
        };
        (range.start < range.end).then_some(range)
    }

    fn selected_text(&self, projections: &[Option<CellSelectionProjection>]) -> Option<String> {
        let (start, end) = self.ordered_points()?;
        let mut selected = String::new();
        for cell in start.cell..=end.cell {
            let projection = projections.get(cell)?.as_ref()?;
            let Some(range) = self.selected_bytes_for_cell(cell, projection.text().len()) else {
                continue;
            };
            let Some(fragment) = projection.text().get(range) else {
                continue;
            };
            if !selected.is_empty() {
                selected.push_str(projection.separator_before);
            }
            selected.push_str(fragment);
        }
        Some(selected)
    }

    fn ordered_points(&self) -> Option<(&SelectionPoint, &SelectionPoint)> {
        let anchor = self.anchor.as_ref()?;
        let focus = self.focus.as_ref()?;
        if (anchor.cell, anchor.bytes.start) <= (focus.cell, focus.bytes.start) {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }
}

#[cfg(test)]
#[path = "conversation_selection_tests.rs"]
mod tests;
