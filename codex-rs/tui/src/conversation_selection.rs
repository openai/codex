//! Source-backed selection for the retained conversation viewport.
//!
//! Mouse coordinates are layout data, not clipboard data. A projection maps rendered terminal
//! cells back to ranges in canonical semantic text; copied output is always sliced from that text.

use std::collections::BTreeMap;
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
    separator_before_is_explicit: bool,
}

/// One rendered child of a composed selection projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CellSelectionProjectionPart {
    Selectable(CellSelectionProjection),
    Transparent { row_count: usize },
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
        let prefix_columns = vec![first_row_prefix_columns; lines.len()];
        Self::from_display_lines_with_prefixes(text, lines, width, &prefix_columns)
    }

    /// Builds a projection with presentation-only prefix widths for each logical display line.
    ///
    /// Each prefix applies only to the first terminal row generated from its logical line because
    /// Ratatui may add further rows while wrapping an overlong token.
    pub(crate) fn from_display_lines_with_prefixes(
        text: String,
        lines: Vec<Line<'static>>,
        width: u16,
        prefix_columns: &[u16],
    ) -> Option<Self> {
        if text.is_empty() || width == 0 || lines.is_empty() || lines.len() != prefix_columns.len()
        {
            return None;
        }

        let semantic = semantic_graphemes(&text);
        let mut semantic_cursor = 0;
        let mut rows = Vec::new();

        for (line_index, line) in lines.into_iter().enumerate() {
            let first_row_prefix_columns = prefix_columns[line_index];
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

                    if let Some((segments, next_column, next_semantic_cursor)) =
                        tab_expansion_segments(
                            &buffer,
                            row_index,
                            column,
                            width,
                            &semantic,
                            semantic_cursor,
                        )
                    {
                        row.segments.extend(segments);
                        column = next_column;
                        semantic_cursor = next_semantic_cursor;
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
            separator_before_is_explicit: false,
        })
    }

    /// Concatenates child projections while preserving presentation-only rows between them.
    pub(crate) fn compose(
        parts: Vec<CellSelectionProjectionPart>,
        blank_rows_between: usize,
        text_separator: &'static str,
    ) -> Option<Self> {
        let mut text = String::new();
        let mut rows = Vec::new();
        let mut has_rendered_part = false;
        let mut has_selectable_part = false;

        for part in parts {
            if has_rendered_part {
                rows.resize(
                    rows.len().saturating_add(blank_rows_between),
                    SelectionRow::default(),
                );
            }

            match part {
                CellSelectionProjectionPart::Selectable(projection) => {
                    if has_selectable_part {
                        text.push_str(text_separator);
                    }
                    let byte_offset = text.len();
                    text.push_str(&projection.text);
                    rows.extend(projection.rows.into_iter().map(|mut row| {
                        for segment in &mut row.segments {
                            segment.bytes = segment.bytes.start.saturating_add(byte_offset)
                                ..segment.bytes.end.saturating_add(byte_offset);
                        }
                        row
                    }));
                    has_selectable_part = true;
                }
                CellSelectionProjectionPart::Transparent { row_count } => {
                    rows.resize(
                        rows.len().saturating_add(row_count),
                        SelectionRow::default(),
                    );
                }
            }
            has_rendered_part = true;
        }

        if !has_selectable_part {
            return None;
        }
        Some(Self {
            text: Arc::from(text),
            rows,
            separator_before: "\n\n",
            separator_before_is_explicit: false,
        })
    }

    /// Builds a projection from an explicit rendered-row mapping.
    ///
    /// This is used for layouts such as wrapped tables, where rendered row-major order differs
    /// from the canonical logical text order and a greedy text alignment cannot recover the
    /// source ranges.
    pub(crate) fn from_rows(text: String, rows: Vec<SelectionRow>) -> Option<Self> {
        if text.is_empty() || rows.is_empty() {
            return None;
        }
        let valid_segment = |segment: &SelectionSegment| {
            segment.columns.start < segment.columns.end
                && segment.bytes.start < segment.bytes.end
                && segment.bytes.end <= text.len()
                && text.is_char_boundary(segment.bytes.start)
                && text.is_char_boundary(segment.bytes.end)
        };
        if rows
            .iter()
            .flat_map(|row| &row.segments)
            .any(|segment| !valid_segment(segment))
            || rows.iter().all(|row| row.segments.is_empty())
        {
            return None;
        }

        Some(Self {
            text: Arc::from(text),
            rows,
            separator_before: "\n\n",
            separator_before_is_explicit: false,
        })
    }

    /// Extracts rendered rows while retaining the semantic gap before their first source glyph.
    ///
    /// Stable streaming cells can split one authored paragraph across several rendered-line
    /// chunks. The next chunk owns the source interval after the previous rendered rows, including
    /// an authored newline when one exists and no synthetic newline when the split was a soft wrap.
    pub(crate) fn slice_rows(&self, row_range: Range<usize>) -> Option<Self> {
        let row_start = row_range.start.min(self.rows.len());
        let row_end = row_range.end.min(self.rows.len());
        if row_start >= row_end {
            return None;
        }

        let preceding_end = self.rows[..row_start]
            .iter()
            .flat_map(|row| &row.segments)
            .map(|segment| segment.bytes.end)
            .max()
            .unwrap_or_default();
        let selected_start = self.rows[row_start..row_end]
            .iter()
            .flat_map(|row| &row.segments)
            .map(|segment| segment.bytes.start)
            .min()?;
        let selected_end = self.rows[row_start..row_end]
            .iter()
            .flat_map(|row| &row.segments)
            .map(|segment| segment.bytes.end)
            .max()?;
        let text_start = if row_start == 0 {
            0
        } else {
            preceding_end.min(selected_start)
        };
        let text_end = if row_end == self.rows.len() {
            self.text.len()
        } else {
            selected_end
        };
        let text = self.text.get(text_start..text_end)?.to_string();
        let mut rows = self.rows[row_start..row_end].to_vec();
        for row in &mut rows {
            row.segments.retain(|segment| {
                segment.bytes.start >= text_start && segment.bytes.end <= text_end
            });
            for segment in &mut row.segments {
                segment.bytes = segment.bytes.start.saturating_sub(text_start)
                    ..segment.bytes.end.saturating_sub(text_start);
            }
        }
        let mut projection = Self::from_rows(text, rows)?;
        projection.separator_before = self.separator_before;
        projection.separator_before_is_explicit = self.separator_before_is_explicit;
        Some(projection)
    }

    /// Shifts source-mapped columns past presentation-only cell chrome.
    pub(crate) fn with_column_offset(mut self, column_offset: u16) -> Self {
        for row in &mut self.rows {
            for segment in &mut row.segments {
                segment.columns = segment.columns.start.saturating_add(column_offset)
                    ..segment.columns.end.saturating_add(column_offset);
            }
        }
        self
    }

    pub(crate) fn with_separator_before(mut self, separator_before: &'static str) -> Self {
        self.separator_before = separator_before;
        self.separator_before_is_explicit = true;
        self
    }

    /// Applies the viewport separator unless the cell already owns its leading source boundary.
    pub(crate) fn with_default_separator_before(mut self, separator_before: &'static str) -> Self {
        if !self.separator_before_is_explicit {
            self.separator_before = separator_before;
        }
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

    fn closest_hit(&self, row: usize, column: u16) -> Option<Range<usize>> {
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

    pub(crate) fn point_range(&self, row: usize, column: u16) -> Range<usize> {
        if let Some(hit) = self.hit(row, column) {
            return hit;
        }
        if let Some(hit) = self.closest_hit(row, column) {
            return hit;
        }

        let preceding =
            self.rows
                .iter()
                .enumerate()
                .take(row)
                .rev()
                .find_map(|(row, selection_row)| {
                    selection_row
                        .segments
                        .last()
                        .map(|segment| (row, segment.bytes.end))
                });
        let following = self
            .rows
            .iter()
            .enumerate()
            .skip(row.saturating_add(1))
            .find_map(|(row, selection_row)| {
                selection_row
                    .segments
                    .first()
                    .map(|segment| (row, segment.bytes.start))
            });
        let gap_start = preceding.map(|(_, bytes)| bytes).unwrap_or_default();
        let gap_end = following.map(|(_, bytes)| bytes).unwrap_or(self.text.len());
        if gap_start >= gap_end {
            let boundary = gap_start.min(self.text.len());
            return boundary..boundary;
        }
        let preceding_row = preceding.map(|(row, _)| row);
        let newline_count = row.saturating_sub(
            preceding_row
                .map(|row| row.saturating_add(1))
                .unwrap_or_default(),
        ) + usize::from(preceding_row.is_some());
        let newline_boundaries = self.text[gap_start..gap_end]
            .match_indices('\n')
            .map(|(offset, newline)| gap_start + offset + newline.len())
            .collect::<Vec<_>>();
        let boundary = if newline_count == 0 {
            gap_start
        } else if let Some(boundary) = newline_boundaries.get(newline_count.saturating_sub(1)) {
            *boundary
        } else {
            newline_boundaries.last().copied().unwrap_or(gap_start)
        };
        boundary..boundary
    }
}

fn tab_expansion_segments(
    buffer: &Buffer,
    row: u16,
    start_column: u16,
    width: u16,
    semantic: &[SemanticGrapheme<'_>],
    semantic_cursor: usize,
) -> Option<(Vec<SelectionSegment>, u16, usize)> {
    let source_run_start = semantic
        .iter()
        .enumerate()
        .skip(semantic_cursor)
        .find(|(_, grapheme)| !is_line_break(grapheme.text))?
        .0;
    let source_run = semantic
        .iter()
        .enumerate()
        .skip(source_run_start)
        .take_while(|(_, grapheme)| is_non_line_break_whitespace(grapheme.text))
        .collect::<Vec<_>>();
    let tab_count = source_run
        .iter()
        .filter(|(_, grapheme)| grapheme.text == "\t")
        .count();
    if tab_count == 0 {
        return None;
    }

    let mut end_column = start_column;
    while end_column < width {
        let cell = &buffer[(end_column, row)];
        let symbol = cell.symbol();
        if cell.skip || symbol.is_empty() || !symbol.chars().all(char::is_whitespace) {
            break;
        }
        let symbol_width = symbol.width().max(1).min(usize::from(u16::MAX)) as u16;
        end_column = end_column.saturating_add(symbol_width).min(width);
    }
    let rendered_columns = usize::from(end_column.saturating_sub(start_column));
    let ordinary_columns = source_run.len().saturating_sub(tab_count);
    if rendered_columns < source_run.len() {
        return None;
    }
    let tab_columns = rendered_columns.saturating_sub(ordinary_columns);
    let base_tab_width = tab_columns / tab_count;
    let wider_tabs = tab_columns % tab_count;
    let mut seen_tabs = 0usize;
    let mut column = start_column;
    let mut segments = Vec::with_capacity(source_run.len());
    for (_, grapheme) in &source_run {
        let columns = if grapheme.text == "\t" {
            let extra = usize::from(seen_tabs < wider_tabs);
            seen_tabs = seen_tabs.saturating_add(1);
            base_tab_width.saturating_add(extra)
        } else {
            1
        };
        let columns = u16::try_from(columns).unwrap_or(u16::MAX);
        let next_column = column.saturating_add(columns).min(end_column);
        segments.push(SelectionSegment {
            columns: column..next_column,
            bytes: grapheme.bytes.clone(),
        });
        column = next_column;
    }

    Some((
        segments,
        end_column,
        source_run_start.saturating_add(source_run.len()),
    ))
}

fn is_line_break(grapheme: &str) -> bool {
    grapheme
        .chars()
        .all(|character| matches!(character, '\n' | '\r'))
}

fn is_non_line_break_whitespace(grapheme: &str) -> bool {
    grapheme
        .chars()
        .all(|character| character.is_whitespace() && !matches!(character, '\n' | '\r'))
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
    if is_non_line_break_whitespace(rendered) {
        return semantic
            .iter()
            .enumerate()
            .skip(cursor)
            .find(|(_, grapheme)| !is_line_break(grapheme.text))
            .and_then(|(index, grapheme)| {
                is_non_line_break_whitespace(grapheme.text).then_some(index)
            });
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

/// A mouse position in the retained conversation's scrollable content coordinates.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct SelectionPoint {
    pub(crate) row: usize,
    pub(crate) column: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectionCellLayout {
    pub(crate) top: usize,
    pub(crate) height: usize,
}

impl SelectionCellLayout {
    fn bottom(&self) -> usize {
        self.top.saturating_add(self.height)
    }

    fn intersects(&self, rows: Range<usize>) -> bool {
        self.height > 0 && self.top < rows.end && self.bottom() > rows.start
    }
}

#[derive(Default)]
pub(crate) struct ConversationSelection {
    anchor: Option<SelectionPoint>,
    focus: Option<SelectionPoint>,
    dragged: bool,
}

impl ConversationSelection {
    pub(crate) fn start(&mut self, point: SelectionPoint) {
        self.anchor = Some(point);
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
        layout: &[SelectionCellLayout],
    ) -> Option<String> {
        self.set_release_point(point);
        let text = self
            .dragged
            .then(|| self.selected_text(projections, layout))
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

    pub(crate) fn endpoints(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        Some((self.anchor?, self.focus?))
    }

    pub(crate) fn remap_endpoints(&mut self, anchor: SelectionPoint, focus: SelectionPoint) {
        self.anchor = Some(anchor);
        self.focus = Some(focus);
    }

    pub(crate) fn selected_cell_span(
        &self,
        layout: &[SelectionCellLayout],
    ) -> Option<Range<usize>> {
        let (start, end) = self.ordered_points()?;
        let selected_rows = start.row..end.row.saturating_add(1);
        let mut selected = layout
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.intersects(selected_rows.clone()))
            .map(|(index, _)| index);
        let first = selected.next()?;
        let last = selected.next_back().unwrap_or(first);
        Some(first..last.saturating_add(1))
    }

    pub(crate) fn segment_is_selected(
        &self,
        layout: SelectionCellLayout,
        row: usize,
        columns: &Range<u16>,
    ) -> bool {
        let Some((start, end)) = self.ordered_points() else {
            return false;
        };
        let content_row = layout.top.saturating_add(row);
        if content_row < start.row || content_row > end.row {
            return false;
        }
        let starts_before_end = content_row < end.row || columns.start <= end.column;
        let ends_after_start = content_row > start.row || columns.end > start.column;
        starts_before_end && ends_after_start
    }

    fn selected_text_for_cell(
        &self,
        projection: &CellSelectionProjection,
        layout: SelectionCellLayout,
    ) -> Option<String> {
        let mut mapped = BTreeMap::<(usize, usize), bool>::new();
        for (row_index, row) in projection.rows().iter().enumerate() {
            for segment in &row.segments {
                let selected = self.segment_is_selected(layout, row_index, &segment.columns);
                mapped
                    .entry((segment.bytes.start, segment.bytes.end))
                    .and_modify(|was_selected| *was_selected |= selected)
                    .or_insert(selected);
            }
        }

        let mut runs = Vec::<Range<usize>>::new();
        let mut current: Option<Range<usize>> = None;
        for ((start, end), selected) in mapped {
            if selected {
                if let Some(run) = current.as_mut() {
                    run.end = run.end.max(end);
                } else {
                    current = Some(start..end);
                }
            } else if let Some(run) = current.take() {
                runs.push(run);
            }
        }
        if let Some(run) = current {
            runs.push(run);
        }

        let (start, end) = self.ordered_points()?;
        if let Some(first) = runs.first_mut() {
            let boundary = if start.row < layout.top {
                0
            } else {
                projection
                    .point_range(start.row.saturating_sub(layout.top), start.column)
                    .start
            };
            if boundary <= first.start
                && projection
                    .text()
                    .get(boundary..first.start)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace))
            {
                first.start = boundary;
            }
        }
        if let Some(last) = runs.last_mut() {
            let boundary = if end.row >= layout.bottom() {
                projection.text().len()
            } else {
                projection
                    .point_range(end.row.saturating_sub(layout.top), end.column)
                    .end
            };
            if boundary >= last.end
                && projection
                    .text()
                    .get(last.end..boundary)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace))
            {
                last.end = boundary;
            }
        }

        let mut selected = String::new();
        for run in runs {
            let Some(fragment) = projection.text().get(run) else {
                continue;
            };
            if !selected.is_empty() {
                selected.push('\n');
            }
            selected.push_str(fragment);
        }
        (!selected.is_empty()).then_some(selected)
    }

    fn selected_text(
        &self,
        projections: &[Option<CellSelectionProjection>],
        layout: &[SelectionCellLayout],
    ) -> Option<String> {
        let selected_cells = self.selected_cell_span(layout)?;
        let mut selected = String::new();
        for cell in selected_cells {
            let Some(projection) = projections.get(cell).and_then(Option::as_ref) else {
                continue;
            };
            let Some(fragment) = layout
                .get(cell)
                .and_then(|layout| self.selected_text_for_cell(projection, *layout))
            else {
                continue;
            };
            if !selected.is_empty() {
                selected.push_str(projection.separator_before);
            }
            selected.push_str(&fragment);
        }
        Some(selected)
    }

    fn ordered_points(&self) -> Option<(&SelectionPoint, &SelectionPoint)> {
        let anchor = self.anchor.as_ref()?;
        let focus = self.focus.as_ref()?;
        if anchor <= focus {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }
}

#[cfg(test)]
#[path = "conversation_selection_tests.rs"]
mod tests;
