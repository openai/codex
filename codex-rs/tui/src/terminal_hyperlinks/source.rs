//! Logical text and margins retained when display wrapping removes whitespace or adds a gutter.

use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::ops::Range;
use std::sync::Arc;

/// Wrapping policy of an existing display renderer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum LineWrapPolicy {
    #[default]
    Word,
}

/// A displayed line's contiguous fragment of one original logical line.
///
/// Wrapped fragments share `text`. `prefix_bytes` counts synthetic display bytes before
/// the fragment, so selection never has to infer whether a gutter belongs to the source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LogicalLineSource {
    pub(crate) text: Arc<str>,
    /// Span styles share the text's byte coordinates, including whitespace omitted by wrapping.
    pub(crate) styles: Arc<[(Range<usize>, Style)]>,
    /// Row style, applied beneath explicit span styles just like ratatui's `Line`.
    pub(crate) line_style: Style,
    /// A later uniform span-style patch, such as the dim/italic reasoning treatment.
    pub(crate) span_style: Style,
    pub(crate) range: Range<usize>,
    pub(crate) prefix_bytes: usize,
    pub(crate) wrap_policy: LineWrapPolicy,
    pub(crate) continuation_indent: Line<'static>,
    /// Blank columns after wrapped content; they remain outside the copied source text.
    pub(crate) right_reserve: u16,
}

impl LogicalLineSource {
    pub(crate) fn new(text: String) -> Self {
        let end = text.len();
        Self {
            text: text.into(),
            styles: vec![(0..end, Style::default())].into(),
            line_style: Style::default(),
            span_style: Style::default(),
            range: 0..end,
            prefix_bytes: 0,
            wrap_policy: LineWrapPolicy::Word,
            continuation_indent: Line::default(),
            right_reserve: 0,
        }
    }

    pub(crate) fn from_line(line: &Line<'_>) -> Self {
        let mut source = Self::new(
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect(),
        );
        let mut offset = 0;
        source.line_style = line.style;
        source.styles = line
            .spans
            .iter()
            .map(|span| {
                let start = offset;
                offset += span.content.len();
                (start..offset, span.style)
            })
            .collect::<Vec<_>>()
            .into();
        source
    }

    #[allow(
        dead_code,
        reason = "Used by the activity presentation layer of this stack."
    )]
    pub(crate) fn styled_range(&self, range: Range<usize>) -> Line<'static> {
        Line::from(
            self.styles
                .iter()
                .filter_map(|(style_range, style)| {
                    let start = style_range.start.max(range.start);
                    let end = style_range.end.min(range.end);
                    (start < end).then(|| {
                        Span::styled(
                            self.text[start..end].to_owned(),
                            self.line_style.patch(*style).patch(self.span_style),
                        )
                    })
                })
                .collect::<Vec<_>>(),
        )
    }

    pub(super) fn wrapped(&self, displayed: Range<usize>, prefix_bytes: usize) -> Self {
        let source_start = self.prefix_bytes;
        let source_end = source_start + self.range.len();
        let start = displayed.start.clamp(source_start, source_end);
        let end = displayed.end.clamp(source_start, source_end);
        Self {
            text: Arc::clone(&self.text),
            styles: Arc::clone(&self.styles),
            line_style: self.line_style,
            span_style: self.span_style,
            range: self.range.start + start - source_start..self.range.start + end - source_start,
            prefix_bytes: prefix_bytes + start.saturating_sub(displayed.start).min(displayed.len()),
            wrap_policy: self.wrap_policy,
            continuation_indent: self.continuation_indent.clone(),
            right_reserve: self.right_reserve,
        }
    }
}
