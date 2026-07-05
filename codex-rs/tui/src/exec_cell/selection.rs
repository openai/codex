//! Source-backed selection metadata for rendered command cells.
//!
//! Command output is wrapped and sometimes truncated before it reaches the retained viewport.
//! These types keep each visible fragment tied to its logical source line so copying can restore
//! authored whitespace skipped at soft-wrap boundaries without including hidden truncation.

use std::ops::Range;
use std::sync::Arc;

use ratatui::text::Line;

use crate::history_cell::SelectionContribution;
use crate::history_cell::selection_contribution_from_semantic_rows;
use crate::history_cell::selection_text_from_lines;

#[derive(Clone, Debug)]
enum ExecSelectionContent {
    Source {
        source_id: usize,
        wrap_index: usize,
        source: Arc<str>,
        visible_bytes: Range<usize>,
        semantic_leading: String,
    },
    Generated(String),
}

/// One logical display line and the semantic content it contributes to selection.
#[derive(Clone, Debug)]
pub(super) struct ExecDisplayLine {
    line: Line<'static>,
    content: ExecSelectionContent,
    prefix_columns: u16,
}

impl ExecDisplayLine {
    pub(super) fn generated(line: Line<'static>, text: String) -> Self {
        Self {
            line,
            content: ExecSelectionContent::Generated(text),
            prefix_columns: 0,
        }
    }

    /// Maps already-wrapped display fragments back to one unwrapped logical source line.
    pub(super) fn from_wrapped_source(
        source_id: usize,
        source_line: &Line<'static>,
        wrapped: Vec<Line<'static>>,
    ) -> Vec<Self> {
        Self::from_wrapped_source_with_continuation_prefix(source_id, source_line, wrapped, "")
    }

    /// Maps wrapped fragments that use a presentation-only continuation indent.
    pub(super) fn from_wrapped_source_with_continuation_prefix(
        source_id: usize,
        source_line: &Line<'static>,
        wrapped: Vec<Line<'static>>,
        continuation_prefix: &str,
    ) -> Vec<Self> {
        let source = selection_text_from_lines(std::slice::from_ref(source_line));
        let source: Arc<str> = Arc::from(source);
        let mut cursor = 0usize;

        wrapped
            .into_iter()
            .enumerate()
            .map(|(wrap_index, line)| {
                let rendered = selection_text_from_lines(std::slice::from_ref(&line));
                let fragment = if wrap_index == 0 {
                    rendered.as_str()
                } else {
                    rendered
                        .strip_prefix(continuation_prefix)
                        .unwrap_or(rendered.as_str())
                };
                let start = source[cursor..]
                    .find(fragment)
                    .map(|offset| cursor.saturating_add(offset))
                    .unwrap_or(cursor);
                let end = start.saturating_add(fragment.len()).min(source.len());
                cursor = end;
                Self {
                    line,
                    content: ExecSelectionContent::Source {
                        source_id,
                        wrap_index,
                        source: Arc::clone(&source),
                        visible_bytes: start..end,
                        semantic_leading: String::new(),
                    },
                    prefix_columns: if wrap_index == 0 {
                        0
                    } else {
                        u16::try_from(Line::from(continuation_prefix).width()).unwrap_or(u16::MAX)
                    },
                }
            })
            .collect()
    }

    pub(super) fn with_prefix(mut self, mut prefix: Line<'static>) -> Self {
        self.prefix_columns = self
            .prefix_columns
            .saturating_add(u16::try_from(prefix.width()).unwrap_or(u16::MAX));
        prefix.spans.append(&mut self.line.spans);
        self.line = prefix;
        self
    }

    /// Prepends a header containing both presentation-only and semantic spans.
    pub(super) fn with_header(
        mut self,
        mut header: Line<'static>,
        presentation_prefix_columns: u16,
        semantic_leading: String,
    ) -> Self {
        self.prefix_columns = presentation_prefix_columns;
        match &mut self.content {
            ExecSelectionContent::Source {
                semantic_leading: existing,
                ..
            } => existing.insert_str(0, &semantic_leading),
            ExecSelectionContent::Generated(text) => text.insert_str(0, &semantic_leading),
        }
        header.spans.append(&mut self.line.spans);
        self.line = header;
        self
    }

    pub(super) fn line(&self) -> &Line<'static> {
        &self.line
    }

    pub(super) fn into_line(self) -> Line<'static> {
        self.line
    }
}

/// A rendered command cell whose clipboard text is assembled from only visible fragments.
pub(super) struct ExecDisplay {
    rows: Vec<ExecDisplayLine>,
}

impl ExecDisplay {
    pub(super) fn new(rows: Vec<ExecDisplayLine>) -> Self {
        Self { rows }
    }

    pub(super) fn into_lines(self) -> Vec<Line<'static>> {
        self.rows
            .into_iter()
            .map(ExecDisplayLine::into_line)
            .collect()
    }

    pub(super) fn selection_contribution(&self, width: u16) -> SelectionContribution {
        let mut text = String::new();
        let mut previous_source: Option<(usize, usize, usize)> = None;

        for row in &self.rows {
            match &row.content {
                ExecSelectionContent::Source {
                    source_id,
                    wrap_index,
                    source,
                    visible_bytes,
                    semantic_leading,
                } => {
                    let continues_previous = previous_source.is_some_and(
                        |(previous_id, previous_wrap, previous_end)| {
                            previous_id == *source_id
                                && previous_wrap.saturating_add(1) == *wrap_index
                                && previous_end <= visible_bytes.start
                        },
                    );
                    if continues_previous {
                        let previous_end = previous_source
                            .map(|(_, _, end)| end)
                            .unwrap_or(visible_bytes.start);
                        if let Some(fragment) = source.get(previous_end..visible_bytes.end) {
                            text.push_str(fragment);
                        }
                    } else {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(semantic_leading);
                        if let Some(fragment) = source.get(visible_bytes.clone()) {
                            text.push_str(fragment);
                        }
                    }
                    previous_source = Some((*source_id, *wrap_index, visible_bytes.end));
                }
                ExecSelectionContent::Generated(fragment) => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(fragment);
                    previous_source = None;
                }
            }
        }

        let lines = self
            .rows
            .iter()
            .map(|row| row.line.clone())
            .collect::<Vec<_>>();
        let prefix_columns = self
            .rows
            .iter()
            .map(|row| row.prefix_columns)
            .collect::<Vec<_>>();
        selection_contribution_from_semantic_rows(text, lines, width, &prefix_columns)
    }
}
