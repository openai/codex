//! Shared history-cell building blocks reused across transcript concerns.

use super::*;
use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::CellSelectionProjectionPart;

#[derive(Debug)]
pub(crate) struct PlainHistoryCell {
    pub(super) lines: Vec<Line<'static>>,
    selection: PlainHistoryCellSelection,
}

#[derive(Debug)]
enum PlainHistoryCellSelection {
    DisplayText,
    Semantic {
        text: String,
        first_row_prefix_columns: Vec<u16>,
    },
}

impl PlainHistoryCell {
    pub(crate) fn new(lines: Vec<Line<'static>>) -> Self {
        Self {
            lines,
            selection: PlainHistoryCellSelection::DisplayText,
        }
    }

    /// Uses semantic clipboard text instead of copying presentation chrome from rendered lines.
    pub(crate) fn with_selection_text(
        mut self,
        text: String,
        first_row_prefix_columns: Vec<u16>,
    ) -> Self {
        self.selection = PlainHistoryCellSelection::Semantic {
            text,
            first_row_prefix_columns,
        };
        self
    }
}

impl HistoryCell for PlainHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.lines.clone())
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        let lines = self.display_lines_for_mode(width, mode);
        match &self.selection {
            PlainHistoryCellSelection::DisplayText => {
                selection_contribution_from_display_lines(lines, width)
            }
            PlainHistoryCellSelection::Semantic {
                text,
                first_row_prefix_columns,
            } => selection_contribution_from_semantic_rows(
                text.clone(),
                lines,
                width,
                first_row_prefix_columns,
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct WebHyperlinkHistoryCell {
    lines: Vec<Line<'static>>,
}

impl WebHyperlinkHistoryCell {
    pub(crate) fn new(lines: Vec<Line<'static>>) -> Self {
        Self { lines }
    }
}

impl HistoryCell for WebHyperlinkHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn display_hyperlink_lines(&self, _width: u16) -> Vec<HyperlinkLine> {
        crate::terminal_hyperlinks::annotate_web_urls(self.lines.clone())
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.lines.clone())
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        selection_contribution_from_display_lines(self.display_lines_for_mode(width, mode), width)
    }
}
#[derive(Debug)]
pub(crate) struct PrefixedWrappedHistoryCell {
    text: Text<'static>,
    initial_prefix: Line<'static>,
    subsequent_prefix: Line<'static>,
}

impl PrefixedWrappedHistoryCell {
    pub(crate) fn new(
        text: impl Into<Text<'static>>,
        initial_prefix: impl Into<Line<'static>>,
        subsequent_prefix: impl Into<Line<'static>>,
    ) -> Self {
        Self {
            text: text.into(),
            initial_prefix: initial_prefix.into(),
            subsequent_prefix: subsequent_prefix.into(),
        }
    }
}

impl HistoryCell for PrefixedWrappedHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let opts = RtOptions::new(width.max(1) as usize)
            .initial_indent(self.initial_prefix.clone())
            .subsequent_indent(self.subsequent_prefix.clone());
        adaptive_wrap_lines(&self.text, opts)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.text.clone().lines)
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let lines = self.display_lines(width);
                let initial_prefix_width =
                    u16::try_from(self.initial_prefix.width()).unwrap_or(u16::MAX);
                let subsequent_prefix_width =
                    u16::try_from(self.subsequent_prefix.width()).unwrap_or(u16::MAX);
                let prefix_columns = (0..lines.len())
                    .map(|index| {
                        if index == 0 {
                            initial_prefix_width
                        } else {
                            subsequent_prefix_width
                        }
                    })
                    .collect::<Vec<_>>();
                selection_contribution_from_semantic_rows(
                    selection_text_from_lines(&self.text.lines),
                    lines,
                    width,
                    &prefix_columns,
                )
            }
        }
    }
}
#[derive(Debug)]
pub(crate) struct CompositeHistoryCell {
    pub(super) parts: Vec<Box<dyn HistoryCell>>,
}

impl CompositeHistoryCell {
    pub(crate) fn new(parts: Vec<Box<dyn HistoryCell>>) -> Self {
        Self { parts }
    }
}

impl HistoryCell for CompositeHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        let mut first = true;
        for part in &self.parts {
            let mut lines = part.display_lines(width);
            if !lines.is_empty() {
                if !first {
                    out.push(Line::from(""));
                }
                out.append(&mut lines);
                first = false;
            }
        }
        out
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut out = Vec::new();
        let mut first = true;
        for part in &self.parts {
            let mut lines = part.display_hyperlink_lines(width);
            if !lines.is_empty() {
                if !first {
                    out.push(HyperlinkLine::from(""));
                }
                out.append(&mut lines);
                first = false;
            }
        }
        out
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut out = Vec::new();
        let mut first = true;
        for part in &self.parts {
            let mut lines = part.transcript_hyperlink_lines(width);
            if !lines.is_empty() {
                if !first {
                    out.push(HyperlinkLine::from(""));
                }
                out.append(&mut lines);
                first = false;
            }
        }
        out
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        let mut first = true;
        for part in &self.parts {
            let mut lines = part.raw_lines();
            if !lines.is_empty() {
                if !first {
                    out.push(Line::from(""));
                }
                out.append(&mut lines);
                first = false;
            }
        }
        out
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        let parts = self
            .parts
            .iter()
            .filter_map(|part| {
                let display_lines = part.display_lines_for_mode(width, mode);
                if display_lines.is_empty() {
                    return None;
                }
                let row_count = Paragraph::new(Text::from(display_lines))
                    .wrap(Wrap { trim: false })
                    .line_count(width);
                Some(match part.selection_contribution(width, mode) {
                    SelectionContribution::Selectable(projection) => {
                        CellSelectionProjectionPart::Selectable(projection)
                    }
                    SelectionContribution::Transparent => {
                        CellSelectionProjectionPart::Transparent { row_count }
                    }
                })
            })
            .collect();
        match CellSelectionProjection::compose(
            parts, /*blank_rows_between*/ 1, /*text_separator*/ "\n\n",
        ) {
            Some(projection) => SelectionContribution::Selectable(projection),
            None => SelectionContribution::Transparent,
        }
    }
}
