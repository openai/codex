//! Row-aware tab projection for owned transcript layout. Source bytes remain unchanged.
//!
//! Tab-free runs use the existing wrapping primitives. Each tab then advances to the next
//! eight-column stop in its actual display row, including the synthetic gutter. Sparse stops
//! are shared by source hit testing, highlighting, and hyperlink projection.

use std::ops::Range;

use ratatui::text::Line;
use ratatui::text::Span;

use crate::line_truncation::line_width;
use crate::terminal_hyperlinks::LineWrapPolicy;
use crate::terminal_hyperlinks::LogicalLineSource;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::WrappedLine;
use crate::wrapping::adaptive_wrap_line_to_width;
use crate::wrapping::adaptive_wrap_line_with_source;
use crate::wrapping::wrap_ranges_trim;

use super::logical::LogicalLine;

#[derive(Default)]
pub(super) struct TabStops(Vec<TabStop>);

struct TabStop {
    offset: usize,
    raw_column: usize,
    width: usize,
    total_width: usize,
}

impl TabStops {
    pub(super) fn column_for_offset(&self, column: usize, offset: usize) -> usize {
        let index = self.0.partition_point(|tab| tab.offset < offset);
        self.project_column(column, index)
    }

    pub(super) fn grapheme_width(&self, offset: usize, grapheme: &str) -> usize {
        if grapheme != "\t" {
            return display_width(grapheme);
        }
        self.0
            .binary_search_by_key(&offset, |tab| tab.offset)
            .map_or(/*default*/ 0, |index| self.0[index].width)
    }

    /// Link starts include tabs immediately before them; ends exclude tabs immediately after.
    pub(super) fn project_link(&self, columns: Range<usize>) -> Range<usize> {
        let start = self
            .0
            .partition_point(|tab| tab.raw_column + display_width("\t") <= columns.start);
        let end = self.0.partition_point(|tab| tab.raw_column < columns.end);
        self.project_column(columns.start, start)..self.project_column(columns.end, end)
    }

    fn project_column(&self, column: usize, tab_count: usize) -> usize {
        let expanded = tab_count
            .checked_sub(/*rhs*/ 1)
            .map_or(/*default*/ 0, |index| self.0[index].total_width);
        column.saturating_sub(tab_count * display_width("\t")) + expanded
    }
}

pub(super) fn wrap_line(
    logical: &LogicalLine,
    width: usize,
) -> (Vec<WrappedLine<'static>>, Vec<TabStops>) {
    let source = LogicalLineSource::from_line(&logical.line.line);
    let continuation = Row::new(
        &logical.subsequent_indent,
        logical.line.line.style,
        /*offset*/ 0,
        width,
    );
    let mut builder = Rows {
        source: &source,
        logical,
        width,
        continuation_width: width.saturating_sub(continuation.column).max(/*other*/ 1),
        current: Row::new(
            &logical.initial_indent,
            logical.line.line.style,
            /*offset*/ 0,
            width,
        ),
        finished: Vec::new(),
    };
    let mut offset = 0;
    for part in source.text.split_inclusive('\t') {
        let run = part.strip_suffix('\t').unwrap_or(part);
        builder.append_run(offset..offset + run.len(), logical.wrap_policy);
        offset += run.len();
        if part.ends_with('\t') {
            // Ordinary wrapping trims trailing spaces. Before a tab they still affect its stop.
            builder.append_run(
                builder.current.wrapped.range.end..offset,
                LineWrapPolicy::Hard,
            );
            builder.append_tab(offset);
            offset += 1;
        }
    }
    builder.finished.push(builder.current);
    builder
        .finished
        .into_iter()
        .map(|row| (row.wrapped, row.tabs))
        .unzip()
}

struct Row {
    wrapped: WrappedLine<'static>,
    tabs: TabStops,
    column: usize,
}

impl Row {
    fn new(
        indent: &Line<'static>,
        style: ratatui::style::Style,
        offset: usize,
        width: usize,
    ) -> Self {
        let mut line = indent.clone().style(style);
        // Gutters are display-only, so expanding their tabs never changes source offsets.
        let mut column = 0;
        for span in &mut line.spans {
            let mut text = String::new();
            for run in span.content.split_inclusive('\t') {
                let body = run.strip_suffix('\t').unwrap_or(run);
                text.push_str(body);
                column += display_width(body);
                if run.ends_with('\t') {
                    let spaces = (8 - column % 8).min(width.saturating_sub(column));
                    text.push_str(&" ".repeat(spaces));
                    column += spaces;
                }
            }
            span.content = text.into();
        }
        let prefix_bytes = line.spans.iter().map(|span| span.content.len()).sum();
        Self {
            wrapped: WrappedLine {
                line,
                range: offset..offset,
                prefix_bytes,
            },
            tabs: TabStops::default(),
            column,
        }
    }
}

struct Rows<'a> {
    source: &'a LogicalLineSource,
    logical: &'a LogicalLine,
    width: usize,
    continuation_width: usize,
    current: Row,
    finished: Vec<Row>,
}

impl Rows<'_> {
    fn next_row(&mut self, offset: usize) {
        let next = Row::new(
            &self.logical.subsequent_indent,
            self.logical.line.line.style,
            offset,
            self.width,
        );
        self.finished
            .push(std::mem::replace(&mut self.current, next));
    }

    fn append_run(&mut self, range: Range<usize>, policy: LineWrapPolicy) {
        if range.is_empty() {
            return;
        }
        if self.current.column >= self.width {
            self.next_row(range.start);
        }
        let text = &self.source.text[range.clone()];
        let mut first_width = self
            .width
            .saturating_sub(self.current.column)
            .max(/*other*/ 1);
        let following_width = self.continuation_width;
        // A URL after a tab may fit the next row even when this row has little room left.
        if policy == LineWrapPolicy::UrlAware
            && first_width < following_width
            && adaptive_wrap_line_with_source(&Line::from(text), RtOptions::new(first_width))
                .first()
                .is_some_and(|row| {
                    line_width(&row.line) > first_width && line_width(&row.line) <= following_width
                })
        {
            self.next_row(range.start);
            first_width = following_width;
        }
        let first = run_ranges(text, first_width, policy)
            .into_iter()
            .next()
            .unwrap_or(0..0);
        let base = match policy {
            LineWrapPolicy::Word | LineWrapPolicy::UrlAware => {
                first.end
                    + text[first.end..]
                        .bytes()
                        .take_while(|byte| *byte == b' ')
                        .count()
            }
            LineWrapPolicy::Hard => first.end,
        };
        let mut ranges = vec![first];
        ranges.extend(
            run_ranges(&text[base..], following_width, policy)
                .into_iter()
                .filter(|range| !range.is_empty())
                .map(|range| base + range.start..base + range.end),
        );
        for (index, part) in ranges.into_iter().enumerate() {
            let part = range.start + part.start..range.start + part.end;
            if index > 0 {
                self.next_row(part.start);
            }
            let line = self.styled_slice(part.clone());
            self.current.column += line.width();
            self.current.wrapped.line.spans.extend(line.spans);
            self.current.wrapped.range.end = part.end;
        }
    }

    fn append_tab(&mut self, offset: usize) {
        if self.current.column >= self.width {
            self.next_row(offset);
        }
        let width =
            (8 - self.current.column % 8).min(self.width.saturating_sub(self.current.column));
        let style = self.styled_slice(offset..offset + 1).spans[0].style;
        let row = &mut self.current;
        let previous_width = row
            .tabs
            .0
            .last()
            .map_or(/*default*/ 0, |tab| tab.total_width);
        row.tabs.0.push(TabStop {
            offset: offset - row.wrapped.range.start,
            raw_column: row.column - previous_width + row.tabs.0.len() * display_width("\t"),
            width,
            total_width: previous_width + width,
        });
        row.wrapped
            .line
            .spans
            .push(Span::styled(" ".repeat(width), style));
        row.wrapped.range.end = offset + 1;
        row.column += width;
    }

    /// Binary-search span boundaries so many runs never rescan the styled source prefix.
    fn styled_slice(&self, range: Range<usize>) -> Line<'static> {
        let first = self
            .source
            .styles
            .partition_point(|(style_range, _)| style_range.end <= range.start);
        Line::from(
            self.source.styles[first..]
                .iter()
                .take_while(|(style_range, _)| style_range.start < range.end)
                .map(|(style_range, style)| {
                    let start = style_range.start.max(range.start);
                    let end = style_range.end.min(range.end);
                    Span::styled(self.source.text[start..end].to_owned(), *style)
                })
                .collect::<Vec<_>>(),
        )
    }
}

fn run_ranges(text: &str, width: usize, policy: LineWrapPolicy) -> Vec<Range<usize>> {
    match policy {
        LineWrapPolicy::Word => wrap_ranges_trim(text, width),
        LineWrapPolicy::UrlAware => {
            adaptive_wrap_line_to_width(&Line::from(text), RtOptions::new(width))
                .into_iter()
                .map(|row| row.range)
                .collect()
        }
        LineWrapPolicy::Hard => {
            let mut offset = 0;
            crate::diff_render::wrap_styled_spans(&[Span::raw(text.to_owned())], width)
                .into_iter()
                .map(|spans| {
                    let start = offset;
                    offset += spans.iter().map(|span| span.content.len()).sum::<usize>();
                    start..offset
                })
                .collect()
        }
    }
}
