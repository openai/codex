//! Render incoming thread messages with literal attribution and a Markdown body.

use super::messages::normalize_whitespace_only_hyperlink_lines;
use super::*;
use crate::markdown_render::ListSpacing;

#[derive(Debug)]
pub(crate) struct DelegatedMessageCell {
    attribution: String,
    markdown: String,
}

impl DelegatedMessageCell {
    pub(crate) fn new(source_thread_id: &str, markdown: String) -> Self {
        Self {
            attribution: format!("Sent by Codex from task {source_thread_id}"),
            markdown,
        }
    }

    fn render_lines(&self, width: u16, list_spacing: ListSpacing) -> Vec<HyperlinkLine> {
        if width == 0 {
            return Vec::new();
        }

        let body_width = width.saturating_sub(2).max(1);
        let indent = "  ";
        let mut lines = crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines(
            &[HyperlinkLine::from(self.attribution.clone())],
            RtOptions::new(usize::from(width))
                .initial_indent("• ".dim().into())
                .subsequent_indent(indent.into()),
        );
        lines.extend(normalize_whitespace_only_hyperlink_lines(
            prefix_hyperlink_lines(
                crate::markdown::render_markdown_for_viewport_with_list_spacing(
                    &self.markdown,
                    body_width,
                    list_spacing,
                ),
                indent.into(),
                indent.into(),
            ),
        ));
        lines
    }
}

impl HistoryCell for DelegatedMessageCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.render_lines(width, ListSpacing::AfterMultiline)
    }

    fn retained_hyperlink_lines(&self, width: u16, _detailed: bool) -> Vec<HyperlinkLine> {
        self.render_lines(width, ListSpacing::Uniform)
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        std::iter::once(Line::from(self.attribution.clone()))
            .chain(
                self.markdown
                    .lines()
                    .map(|line| Line::from(line.to_owned())),
            )
            .collect()
    }
}
