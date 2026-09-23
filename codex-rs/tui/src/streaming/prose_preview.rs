//! Bounded, disposable previews of unterminated prose. Preview lines never enter scrollback;
//! newline commitment and finalization render the original source independently. Math previews
//! show wrapped source until closure, without interpreting TeX punctuation as Markdown.
//! Rich prose withholds unfinished link destinations so closing a link cannot collapse URL rows.

use super::render::render_source;
use crate::history_cell::HistoryRenderMode;
use crate::inline_visualization::InlineVisualizationContext;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::lines_with_sources_eq;
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
    rich_safe_len: usize,
    has_pipe: bool,
    link: LinkPreview,
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
        self.link.scan(source, self.scanned_len);
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
            self.rich_safe_len = source.len();
        }
        // Keep Raw/Math's structural bound independent of Rich-only link holdback.
        if let Some((start, _)) = self.link.destination {
            self.rich_safe_len = self.rich_safe_len.min(start);
        }
        // Retain the last safe text when tokens reveal structure, but still reflow it.
        let source = &source[..if matches!(mode, PreviewMode::Prose(HistoryRenderMode::Rich)) {
            self.rich_safe_len
        } else {
            self.safe_len
        }];
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
        if lines_with_sources_eq(&self.lines, &lines) {
            return false;
        }
        self.lines = lines;
        true
    }
}

/// Scans append-only source without buffering or rescanning a growing destination.
/// Keep the label visible; the opening `(` and everything after it wait for closure.
#[derive(Default)]
struct LinkPreview {
    brackets: usize,
    closed_label: bool,
    destination: Option<(usize, usize)>,
    delimiter: Option<u8>,
    whitespace: bool,
    escaped: bool,
    code_ticks: usize,
    pending_ticks: usize,
}

impl LinkPreview {
    fn scan(&mut self, source: &str, start: usize) {
        for (index, byte) in source.bytes().enumerate().skip(start) {
            if self.destination.is_none() {
                if byte == b'`' && (self.code_ticks > 0 || !self.escaped) {
                    self.pending_ticks += 1;
                    self.closed_label = false;
                    continue;
                }
                if self.pending_ticks > 0 {
                    if self.code_ticks == 0 {
                        self.code_ticks = self.pending_ticks;
                    } else if self.code_ticks == self.pending_ticks {
                        self.code_ticks = 0;
                    }
                    self.pending_ticks = 0;
                }
                if self.code_ticks > 0 {
                    continue;
                }
            }
            if self.escaped {
                self.escaped = false;
                self.closed_label = false;
                self.whitespace = false;
                continue;
            }
            if byte == b'\\' {
                self.escaped = true;
                self.closed_label = false;
                continue;
            }
            if let Some((destination_start, depth)) = self.destination.as_mut() {
                if let Some(delimiter) = self.delimiter {
                    if byte == delimiter {
                        self.delimiter = None;
                    }
                } else if byte == b'<'
                    && *depth == 1
                    && (index == *destination_start + 1 || self.whitespace)
                {
                    self.delimiter = Some(b'>');
                } else if matches!(byte, b'\'' | b'"') && self.whitespace && *depth == 1 {
                    self.delimiter = Some(byte);
                } else if byte == b'(' {
                    *depth += 1;
                } else if byte == b')' {
                    *depth -= 1;
                    if *depth == 0 {
                        self.destination = None;
                    }
                }
                self.whitespace = byte.is_ascii_whitespace();
            } else {
                match byte {
                    b'[' => self.brackets += 1,
                    b']' if self.brackets > 0 => {
                        self.brackets -= 1;
                        self.closed_label = true;
                        continue;
                    }
                    b'(' if self.closed_label => {
                        self.destination = Some((index, 1));
                        self.whitespace = false;
                    }
                    _ => {}
                }
                self.closed_label = false;
            }
        }
    }
}
