//! Reconstruct logical styled lines from explicit wrapping provenance for resize and copy.

use std::ops::Range;
use std::sync::Arc;

use ratatui::text::Line;
use ratatui::text::Span;

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LineWrapPolicy;
use crate::terminal_hyperlinks::LogicalLineSource;
use crate::width::display_width;

#[derive(Clone)]
pub(super) struct LogicalLine {
    pub(super) line: HyperlinkLine,
    pub(super) initial_indent: Line<'static>,
    pub(super) subsequent_indent: Line<'static>,
    pub(super) wrap_policy: LineWrapPolicy,
    pub(super) right_reserve: u16,
    pub(super) origin: LogicalLineSource,
}

pub(super) fn logical_lines(lines: &[HyperlinkLine]) -> Vec<LogicalLine> {
    let mut logical: Vec<LogicalLine> = Vec::new();
    for line in lines {
        let source = line
            .source
            .clone()
            .unwrap_or_else(|| LogicalLineSource::from_line(&line.line));
        let previous = logical.last_mut().filter(|previous| {
            Arc::ptr_eq(&previous.origin.text, &source.text)
                && previous.origin.range.end <= source.range.start
        });
        match previous {
            Some(previous) => previous.append(line, source),
            None => {
                let mut entry = LogicalLine {
                    line: HyperlinkLine::new(Line::default().style(line.line.style)),
                    initial_indent: slice_line(&line.line, 0..source.prefix_bytes),
                    subsequent_indent: source.continuation_indent.clone(),
                    wrap_policy: source.wrap_policy,
                    right_reserve: source.right_reserve,
                    origin: source.clone(),
                };
                entry.origin.range.end = source.range.start;
                entry.append(line, source);
                logical.push(entry);
            }
        }
    }
    logical
}

impl LogicalLine {
    fn append(&mut self, line: &HyperlinkLine, source: LogicalLineSource) {
        self.line.line.spans.extend(
            source
                .styled_range(self.origin.range.end..source.range.start)
                .spans,
        );
        let column = self.line.width();
        let body = slice_line(
            &line.line,
            source.prefix_bytes..source.prefix_bytes + source.range.len(),
        );
        let prefix = slice_line(&line.line, 0..source.prefix_bytes);
        let prefix_width = prefix.width();
        let content_width = display_width(&source.text[source.range.clone()]);
        self.line.line.spans.extend(body.spans);
        self.line
            .hyperlinks
            .extend(line.hyperlinks.iter().filter_map(|link| {
                let start = link.columns.start.max(prefix_width);
                let end = link.columns.end.min(prefix_width + content_width);
                (start < end).then(|| {
                    link.with_columns(column + start - prefix_width..column + end - prefix_width)
                })
            }));
        self.line.line.alignment = line.line.alignment;
        self.origin.range.end = source.range.end;
    }
}

pub(super) fn slice_line(line: &Line<'_>, range: Range<usize>) -> Line<'static> {
    let mut offset = 0;
    let spans = line
        .spans
        .iter()
        .filter_map(|span| {
            let start = range.start.saturating_sub(offset).min(span.content.len());
            let end = range.end.saturating_sub(offset).min(span.content.len());
            offset += span.content.len();
            (start < end).then(|| {
                Span::styled(
                    span.content[start..end].to_owned(),
                    line.style.patch(span.style),
                )
            })
        })
        .collect::<Vec<_>>();
    Line::from(spans).style(line.style)
}
