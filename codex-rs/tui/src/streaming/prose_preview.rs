//! Bounded, disposable previews of unterminated prose. Preview lines never enter scrollback;
//! newline commitment and finalization render the original source independently. Math previews
//! show wrapped source until closure, without interpreting TeX punctuation as Markdown.

use super::render::render_source;
use crate::history_cell::HistoryRenderMode;
use crate::inline_visualization::InlineVisualizationContext;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::text::Line;
use std::path::Path;

const MAX_PREVIEW_BYTES: usize = 8192;

pub(super) enum PreviewMode {
    Prose(HistoryRenderMode),
    Math,
}

/// Tracks one append-only incomplete line; reset when a newline is committed.
#[derive(Default)]
pub(super) struct ProsePreview {
    pub(super) lines: Vec<HyperlinkLine>,
    scanned_len: usize,
    safe_len: usize,
    has_pipe: bool,
}

impl ProsePreview {
    pub(super) fn update(
        &mut self,
        source: &str,
        width: Option<usize>,
        cwd: &Path,
        mode: PreviewMode,
        inline_visualization_context: Option<&InlineVisualizationContext>,
    ) -> bool {
        // Only scan newly arrived bytes, including on very long single-line responses.
        self.has_pipe |= source[self.scanned_len..].contains('|');
        self.scanned_len = source.len();
        // Indented and quoted lines may belong to nested code blocks. Keep their
        // existing newline holdback instead of guessing at the missing block context.
        if matches!(mode, PreviewMode::Math)
            || !(self.has_pipe
                || source.starts_with([' ', '\t', '>'])
                || source.starts_with("```")
                || source.starts_with("~~~")
                || matches!(source, "`" | "``" | "~" | "~~"))
        {
            self.safe_len = source.len();
        }
        // Retain the last safe text when tokens reveal structure, but still reflow it.
        let source = &source[..self.safe_len];
        let start = source.ceil_char_boundary(source.len().saturating_sub(MAX_PREVIEW_BYTES));
        let mut lines = match mode {
            PreviewMode::Prose(render_mode) => render_source(
                &source[start..],
                width,
                cwd,
                render_mode,
                inline_visualization_context,
            ),
            PreviewMode::Math => textwrap::wrap(&source[start..], width.unwrap_or(usize::MAX))
                .into_iter()
                .map(|line| HyperlinkLine::new(Line::from(line.into_owned())))
                .collect(),
        };
        if start > 0 {
            lines.insert(0, HyperlinkLine::new(Line::from("…")));
        }
        if self.lines == lines {
            return false;
        }
        self.lines = lines;
        true
    }
}
