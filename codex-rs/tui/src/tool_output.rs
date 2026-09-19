//! Compact tool output previews. Keep three screen rows and report hidden logical lines below
//! them; a partially displayed logical line counts as hidden. Full output belongs in the transcript.
//! Preserve logical source ranges and hyperlinks while bounding work before display wrapping.

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::live_wrap::take_prefix_by_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::LogicalLineSource;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::remap_source_wrapped_line;
use crate::ui_consts::TRANSCRIPT_HINT;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line_with_source;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::WordSplitter;

const PREVIEW_LINES: usize = 3;
const MAX_PREVIEW_LINE_BYTES: usize = 16 * 1024;

/// Render a bounded preview with source ranges retained for transcript selection and copying.
pub(crate) fn tool_output_hyperlink_preview<'a>(
    lines: impl IntoIterator<Item = Line<'a>>,
    width: usize,
    total_lines: usize,
) -> Vec<HyperlinkLine> {
    let mut preview = ToolOutputPreview::new(width, /*omitted*/ 0);
    let mut consumed = 0;
    for line in lines.into_iter().take(PREVIEW_LINES) {
        preview.push_line(line);
        consumed += 1;
    }
    preview.omitted += total_lines.saturating_sub(consumed);
    preview.finish_hyperlink_lines()
}

pub(crate) struct ToolOutputPreview {
    lines: Vec<HyperlinkLine>,
    width: usize,
    omitted: usize,
    full: bool,
}

impl ToolOutputPreview {
    pub(crate) fn new(width: usize, omitted: usize) -> Self {
        Self {
            lines: Vec::new(),
            width: width.max(/*other*/ 1),
            omitted,
            full: false,
        }
    }

    pub(crate) fn push_line(&mut self, line: Line<'_>) {
        self.push_rendered_line(line, /*source*/ None, Vec::new());
    }

    pub(crate) fn push_hyperlink_line(&mut self, line: HyperlinkLine) {
        self.push_rendered_line(line.line, line.source, line.hyperlinks);
    }

    fn push_rendered_line(
        &mut self,
        line: Line<'_>,
        source: Option<LogicalLineSource>,
        hyperlinks: Vec<TerminalHyperlink>,
    ) {
        let remaining = PREVIEW_LINES - self.lines.len();
        if self.full || remaining == 0 {
            self.omitted += 1;
            return;
        }
        // Bound the input before wrapping: a single tool-result line can be megabytes long.
        // Keep one extra row so cutting a word cannot change the last visible row's wrapping.
        let mut columns = self.width.saturating_mul(remaining + 1);
        // Zero-width characters still cost bytes and work, even within one grapheme.
        let mut bytes = MAX_PREVIEW_LINE_BYTES;
        let mut bounded = Line::default().style(line.style);
        let mut truncated = false;
        for span in &line.spans {
            let text = &span.content[..span.content.floor_char_boundary(bytes)];
            let (prefix, rest, used) = take_prefix_by_width(text, columns);
            bytes -= prefix.len();
            bounded.push_span(Span::styled(prefix.to_owned(), span.style));
            columns = columns.saturating_sub(used);
            if !rest.is_empty() || text.len() < span.content.len() {
                truncated = true;
                break;
            }
        }
        // Hard-wrap even URL tokens here so the preview cannot exceed its row budget, while
        // remapping each visible fragment to the original hyperlink destination and source.
        let bounded = HyperlinkLine {
            line: bounded,
            hyperlinks,
            source,
        };
        let wrapped = word_wrap_line_with_source(
            &bounded.line,
            RtOptions::new(self.width).word_splitter(WordSplitter::NoHyphenation),
        );
        if truncated || wrapped.len() > remaining {
            self.omitted += 1;
            self.full = true;
        }
        self.lines.extend(remap_source_wrapped_line(
            &bounded,
            wrapped.into_iter().take(remaining).collect(),
        ));
    }

    pub(crate) fn finish_hyperlink_lines(mut self) -> Vec<HyperlinkLine> {
        if self.omitted > 0 {
            let omitted = self.omitted;
            let unit = if omitted == 1 { "line" } else { "lines" };
            self.lines.push(
                truncate_line_with_ellipsis_if_overflow(
                    format!("+{omitted} {unit} ({TRANSCRIPT_HINT})")
                        .dim()
                        .into(),
                    self.width,
                )
                .into(),
            );
        }
        self.lines
    }
}

#[cfg(test)]
#[path = "tool_output_tests.rs"]
mod tests;
