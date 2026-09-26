//! Wrapped transcript text with source positions shared by painting and selection.
//!
//! Input lines with shared source provenance belong to the same logical line. Other input
//! lines introduce hard breaks. Display wrapping and synthetic controls never enter `text`.
//! Layout offsets use `usize`; terminal coordinates are narrowed only for visible rows.
//! Disclosure controls follow their activity's source text without changing its indentation.
//! Selected hard breaks highlight one trailing cell when space permits.

use std::borrow::Cow;
use std::ops::Range;

#[path = "text_logical.rs"]
mod logical;

#[path = "text_tabs.rs"]
mod tabs;

use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;

use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_to_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use crate::terminal_hyperlinks::LineWrapPolicy;
use crate::terminal_hyperlinks::LogicalLineSource;
use crate::terminal_hyperlinks::remap_source_wrapped_line;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::WrappedLine;
use crate::wrapping::adaptive_wrap_line_to_width;
use crate::wrapping::word_wrap_line_with_source;
use logical::LogicalLine;
use logical::logical_lines;
use ratatui::text::Line;

/// One cell's logical text and its width-dependent display rows.
pub(super) struct TextLayout {
    logical: Vec<LogicalLine>,
    text: String,
    rows: Vec<TextRow>,
    width: u16,
    pub(super) separated: bool,
    pub(super) disclosure: bool,
    disclosure_control: Option<DisclosureControl>,
}

struct DisclosureControl {
    label: String,
    source_offset: usize,
    row: usize,
}

struct TextRow {
    line: HyperlinkLine,
    source: Range<usize>,
    /// Source offset at the logical line end; absent on soft wraps and synthetic rows.
    line_end: Option<usize>,
    content_width: u16,
    first_column: usize,
    prefix_columns: usize,
    tabs: tabs::TabStops,
}

impl TextLayout {
    pub(super) fn new(lines: Vec<HyperlinkLine>, width: u16) -> Self {
        let logical = logical_lines(&lines);
        Self::from_logical(logical, width)
    }

    /// Reflow the same displayed content revision, retaining styles, links and disclosure label.
    pub(super) fn rewrap(&self, width: u16) -> Self {
        let mut layout = Self::from_logical(self.logical.clone(), width);
        if let Some(control) = &self.disclosure_control {
            layout =
                layout.with_disclosure_control_at(control.label.clone(), control.source_offset);
        }
        if self.separated {
            layout.with_leading_separator()
        } else {
            layout
        }
    }

    /// Append a clipped interaction row outside the copied and searched source text.
    pub(super) fn with_disclosure_control(self, label: String) -> Self {
        let source_offset = self.text.len();
        self.with_disclosure_control_at(label, source_offset)
    }

    /// Place a control after an activity, before any following auxiliary source text.
    pub(super) fn with_disclosure_control_at(
        mut self,
        label: String,
        source_offset: usize,
    ) -> Self {
        if self.text.is_empty() || label.is_empty() {
            return self;
        }
        if let Some(control) = self.disclosure_control.take() {
            self.rows.remove(control.row);
        }
        let source_offset = self
            .text
            .floor_char_boundary(source_offset.min(self.text.len()));
        let control_row = self
            .rows
            .iter()
            .rposition(|row| row.source.end <= source_offset)
            .map_or(/*default*/ 0, |row| row + 1);
        let indent = usize::from(self.width / 4).min(/*other*/ 4);
        let line = truncate_line_to_width(
            Line::from(format!("{}{label}", " ".repeat(indent))).dim(),
            usize::from(self.width),
        );
        self.rows.insert(
            control_row,
            TextRow {
                line: HyperlinkLine::from(line),
                source: source_offset..source_offset,
                line_end: None,
                content_width: self.width,
                first_column: indent,
                prefix_columns: indent,
                tabs: tabs::TabStops::default(),
            },
        );
        self.disclosure = true;
        self.disclosure_control = Some(DisclosureControl {
            label,
            source_offset,
            row: control_row,
        });
        self
    }

    /// Add visual spacing between entries without changing any source position.
    pub(super) fn with_leading_separator(mut self) -> Self {
        if !self.separated {
            self = self.with_leading_spacer();
            self.separated = !self.rows.is_empty();
        }
        self
    }

    /// Reserve one presentation-only row before existing spacing and source text.
    pub(super) fn with_leading_spacer(mut self) -> Self {
        if !self.rows.is_empty() {
            self.rows.insert(
                /*index*/ 0,
                TextRow {
                    line: HyperlinkLine::from(""),
                    source: 0..0,
                    line_end: None,
                    content_width: self.width,
                    first_column: 0,
                    prefix_columns: 0,
                    tabs: tabs::TabStops::default(),
                },
            );
            if let Some(control) = &mut self.disclosure_control {
                control.row += 1;
            }
        }
        self
    }

    /// Source text between adjacent retained fragments; distinct logical lines have a hard break.
    pub(super) fn separator_after(&self, next: &Self) -> &str {
        if let Some(previous) = self.logical.last()
            && let Some(next) = next.logical.first()
            && std::sync::Arc::ptr_eq(&previous.origin.text, &next.origin.text)
            && let Some(gap) = previous
                .origin
                .text
                .get(previous.origin.range.end..next.origin.range.start)
        {
            gap
        } else {
            "\n"
        }
    }

    pub(super) fn copy_lines(
        &self,
        range: Range<usize>,
        separator: &str,
        output: &mut Vec<crate::markdown_copy::SelectedLine>,
    ) {
        let mut offset = 0;
        for (index, line) in self.logical.iter().enumerate() {
            let end = offset + line.origin.range.len();
            if range.start <= end && range.end >= offset {
                let selected = line.origin.range.start + range.start.saturating_sub(offset)
                    ..line.origin.range.start + range.end.min(end) - offset;
                crate::markdown_copy::SelectedLine::append(
                    output,
                    line.origin.clone(),
                    selected,
                    if index == 0 { separator } else { "\n" },
                );
            }
            offset = end + 1;
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn disclosure_row(&self) -> Option<usize> {
        self.disclosure_control.as_ref().map(|control| control.row)
    }

    pub(super) fn disclosure_columns(&self) -> Range<u16> {
        let Some(row) = self.disclosure_row().and_then(|row| self.rows.get(row)) else {
            return 0..0;
        };
        row.first_column as u16..line_width(&row.line.line).min(usize::from(self.width)) as u16
    }

    /// Paint visible rows, including inherited styles on empty rows and trailing columns.
    pub(super) fn render(&self, area: Rect, buf: &mut Buffer, start_row: usize) {
        for (screen_row, row) in self.visible_rows(area, start_row) {
            let mut row_area =
                Rect::new(area.x, area.y + screen_row, area.width, /*height*/ 1);
            buf.set_style(row_area, row.line.line.style);
            row_area.width = row.content_width;
            HyperlinkParagraph::new(std::slice::from_ref(&row.line), row.line.line.style)
                .render(row_area, buf);
        }
    }

    /// Resolve a cell coordinate to a grapheme boundary, clamping padding to the row's text.
    pub(super) fn position_at(&self, row: usize, column: u16) -> usize {
        let Some(row) = self.rows.get(row) else {
            return self.text.len();
        };
        let mut current_column = row.first_column;
        for (offset, grapheme) in
            self.text[row.source.clone()].grapheme_indices(/*is_extended*/ true)
        {
            current_column += row.tabs.grapheme_width(offset, grapheme);
            if usize::from(column) < current_column {
                return row.source.start + offset;
            }
        }
        row.source.end
    }

    /// Find the display row containing a source position, including omitted wrapping whitespace.
    pub(super) fn row_for_offset(&self, offset: usize) -> usize {
        let before_control = if let Some(control_row) = self.disclosure_row() {
            let after_control = &self.rows[control_row + 1..];
            let matching_rows = after_control.partition_point(|row| row.source.start <= offset);
            if matching_rows > 0 {
                return control_row + matching_rows;
            }
            control_row
        } else {
            self.rows.len()
        };
        self.rows[..before_control]
            .partition_point(|row| row.source.start <= offset)
            .saturating_sub(/*rhs*/ 1)
    }

    pub(super) fn column_for_offset(&self, offset: usize) -> u16 {
        let Some(row) = self.rows.get(self.row_for_offset(offset)) else {
            return 0;
        };
        let offset = self
            .text
            .floor_char_boundary(offset.clamp(row.source.start, row.source.end));
        let column = row.first_column
            + row.tabs.column_for_offset(
                display_width(&self.text[row.source.start..offset]),
                offset - row.source.start,
            );
        column.min(usize::from(self.width)) as u16
    }

    /// Highlight source graphemes only, leaving alignment and viewport padding untouched.
    pub(super) fn highlight(
        &self,
        range: Range<usize>,
        area: Rect,
        buf: &mut Buffer,
        start_row: usize,
    ) {
        for (screen_row, row) in self.visible_rows(area, start_row) {
            let mut column = row.first_column;
            for (offset, grapheme) in
                self.text[row.source.clone()].grapheme_indices(/*is_extended*/ true)
            {
                let start = row.source.start + offset;
                let width = row.tabs.grapheme_width(offset, grapheme);
                if start < range.end && start + grapheme.len() > range.start {
                    for selected_column in
                        column..(column + width).min(usize::from(row.content_width))
                    {
                        buf[(area.x + selected_column as u16, area.y + screen_row)]
                            .set_style(Style::default().add_modifier(Modifier::REVERSED));
                    }
                }
                column += width;
            }
        }
    }

    /// Add a trailing cell for selected hard breaks, including a copied separator to `next`.
    pub(super) fn highlight_selection(
        &self,
        range: Range<usize>,
        next: Option<&Self>,
        area: Rect,
        buf: &mut Buffer,
        start_row: usize,
    ) {
        self.highlight(range.clone(), area, buf, start_row);
        for (screen_row, row) in self.visible_rows(area, start_row) {
            let Some(end) = row.line_end else { continue };
            let selected = range.contains(&end)
                || (end == self.text.len()
                    && next.is_some_and(|next| self.separator_after(next).contains('\n')));
            let column = row.first_column
                + row.tabs.column_for_offset(
                    display_width(&self.text[row.source.clone()]),
                    row.source.len(),
                );
            if selected && column < usize::from(self.width) {
                buf[(area.x + column as u16, area.y + screen_row)]
                    .set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }
    }

    /// Resolve the displayed link using the same destination policy as terminal OSC-8 output.
    pub(super) fn link_at(&self, row: usize, column: u16) -> Option<String> {
        let row = self.rows.get(row)?;
        let column = usize::from(column).checked_sub(row.first_column - row.prefix_columns)?;
        row.line
            .hyperlinks
            .iter()
            .find(|link| link.columns.contains(&column))?
            .terminal_destination()
    }

    fn visible_rows(&self, area: Rect, start_row: usize) -> impl Iterator<Item = (u16, &TextRow)> {
        let height = if area.width == 0 { 0 } else { area.height };
        debug_assert!(area.width == 0 || area.width == self.width);
        self.rows
            .iter()
            .skip(start_row)
            .take(usize::from(height))
            .enumerate()
            .map(|(row, text)| (row as u16, text))
    }

    fn from_logical(logical: Vec<LogicalLine>, width: u16) -> Self {
        let width = width.max(/*other*/ 1);
        let mut text = String::new();
        let mut rows = Vec::new();
        for (index, line) in logical.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            let start = text.len();
            text.extend(
                line.line
                    .line
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref()),
            );
            rows.extend(layout_line(line, start, width));
            if let Some(row) = rows.last_mut() {
                row.line_end = Some(text.len());
            }
        }
        Self {
            logical,
            text,
            rows,
            width,
            separated: false,
            disclosure: false,
            disclosure_control: None,
        }
    }
}

fn layout_line(logical: &LogicalLine, text_start: usize, width: u16) -> Vec<TextRow> {
    let content_width = width.saturating_sub(logical.right_reserve).max(/*other*/ 1);
    let mut logical = Cow::Borrowed(logical);
    if line_width(&logical.initial_indent) >= usize::from(content_width)
        || line_width(&logical.subsequent_indent) >= usize::from(content_width)
    {
        let logical = logical.to_mut();
        logical.initial_indent = Line::default();
        logical.subsequent_indent = Line::default();
    }
    let mut line = logical.line.clone();
    line.source = Some(LogicalLineSource::new(
        line.line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect(),
    ));
    let options = RtOptions::new(usize::from(content_width))
        .initial_indent(logical.initial_indent.clone())
        .subsequent_indent(logical.subsequent_indent.clone());
    let (wrapped, tab_stops) = if line
        .line
        .spans
        .iter()
        .any(|span| span.content.contains('\t'))
    {
        tabs::wrap_line(&logical, usize::from(content_width))
    } else {
        let wrapped = match logical.wrap_policy {
            LineWrapPolicy::Word => word_wrap_line_with_source(&line.line, options),
            LineWrapPolicy::UrlAware => adaptive_wrap_line_to_width(&line.line, options),
            LineWrapPolicy::Hard => hard_wrap_line(&line.line, options),
        };
        let stops = (0..wrapped.len())
            .map(|_| tabs::TabStops::default())
            .collect();
        (wrapped, stops)
    };
    // This source covers the whole logical line, so wrapping already supplies final row offsets.
    let positions = wrapped
        .iter()
        .map(|row| (row.range.clone(), row.prefix_bytes))
        .collect::<Vec<_>>();
    remap_source_wrapped_line(&line, wrapped)
        .into_iter()
        .zip(positions)
        .zip(tab_stops)
        .map(|((mut wrapped, (source, prefix_bytes)), tabs)| {
            wrapped.line.alignment = line.line.alignment;
            for link in &mut wrapped.hyperlinks {
                link.columns = tabs.project_link(link.columns.clone());
            }
            let displayed: String = wrapped
                .line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            let prefix_columns = display_width(&displayed[..prefix_bytes]);
            let columns = usize::from(content_width);
            let text_width = display_width(&displayed);
            let first_column = match line.line.alignment.unwrap_or(Alignment::Left) {
                Alignment::Left => 0,
                Alignment::Center => (columns / 2).saturating_sub(text_width / 2),
                Alignment::Right => columns.saturating_sub(text_width),
            };
            let range = text_start + source.start..text_start + source.end;
            TextRow {
                line: wrapped,
                source: range,
                line_end: None,
                content_width,
                first_column: first_column + prefix_columns,
                prefix_columns,
                tabs,
            }
        })
        .collect()
}

/// Reuse the diff renderer's hard wrapping while preserving its distinct first/continuation gutters.
fn hard_wrap_line(line: &Line<'static>, options: RtOptions<'static>) -> Vec<WrappedLine<'static>> {
    let first_width = options
        .width
        .saturating_sub(options.initial_indent.width())
        .max(/*other*/ 1);
    let first = crate::diff_render::wrap_styled_spans(&line.spans, first_width)
        .into_iter()
        .next()
        .unwrap_or_default();
    let mut offset = first.iter().map(|span| span.content.len()).sum();
    let prefix_bytes = options
        .initial_indent
        .spans
        .iter()
        .map(|span| span.content.len())
        .sum();
    let mut initial = options.initial_indent.clone().style(line.style);
    initial.spans.extend(first);
    let mut out = vec![WrappedLine {
        line: initial,
        range: 0..offset,
        prefix_bytes,
    }];
    let total = line.spans.iter().map(|span| span.content.len()).sum();
    let remainder = logical::slice_line(line, offset..total);
    let continuation_width = options
        .width
        .saturating_sub(options.subsequent_indent.width())
        .max(/*other*/ 1);
    for chunk in crate::diff_render::wrap_styled_spans(&remainder.spans, continuation_width) {
        let length: usize = chunk.iter().map(|span| span.content.len()).sum();
        if length == 0 {
            continue;
        }
        let mut continuation = options.subsequent_indent.clone().style(line.style);
        let prefix_bytes = continuation
            .spans
            .iter()
            .map(|span| span.content.len())
            .sum();
        continuation.spans.extend(chunk);
        out.push(WrappedLine {
            line: continuation,
            range: offset..offset + length,
            prefix_bytes,
        });
        offset += length;
    }
    out
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "diff_source_tests.rs"]
mod diff_source_tests;

#[cfg(test)]
#[path = "source_cell_tests.rs"]
mod source_cell_tests;

#[cfg(test)]
#[path = "prompt_style_tests.rs"]
mod prompt_style_tests;

#[cfg(test)]
#[path = "text_tabs_tests.rs"]
mod tab_tests;
