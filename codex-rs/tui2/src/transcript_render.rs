//! Transcript rendering helpers (flattening, wrapping, and metadata).
//!
//! `App` treats the transcript (history cells) as the source of truth and
//! renders a *flattened* list of visual lines into the viewport. A single
//! history cell may render multiple visual lines, and the viewport may include
//! synthetic spacer rows between cells.
//!
//! This module centralizes the logic for:
//! - Flattening history cells into visual `ratatui::text::Line`s.
//! - Producing parallel metadata (`TranscriptLineMeta`) used for scroll
//!   anchoring and "user row" styling.
//! - Computing *soft-wrap joiners* so copy can treat wrapped prose as one
//!   logical line instead of inserting hard newlines.

use crate::history_cell::HistoryCell;
use crate::tui::scrolling::TranscriptLineMeta;
use ratatui::text::Line;
use std::sync::Arc;

/// Flattened transcript lines plus the metadata required to interpret them.
#[derive(Debug)]
pub(crate) struct TranscriptLines {
    /// Flattened visual transcript lines, in the same order they are rendered.
    pub(crate) lines: Vec<Line<'static>>,
    /// Parallel metadata for each line (same length as `lines`).
    ///
    /// This maps a visual line back to `(cell_index, line_in_cell)` so scroll
    /// anchoring and "user row" styling remain stable across reflow.
    pub(crate) meta: Vec<TranscriptLineMeta>,
    /// Soft-wrap joiners (same length as `lines`).
    ///
    /// `joiner_before[i]` is `Some(joiner)` when line `i` is a soft-wrap
    /// continuation of line `i - 1`, and `None` when the break is a hard break
    /// (between input lines/cells, or spacer rows).
    ///
    /// Copy uses this to join wrapped prose without inserting hard newlines,
    /// while still preserving hard line breaks and explicit blank lines.
    pub(crate) joiner_before: Vec<Option<String>>,
}

/// Build flattened transcript lines without applying additional viewport wrapping.
///
/// This is useful for:
/// - Exit transcript rendering (ANSI) where we want the "cell as rendered"
///   output.
/// - Any consumer that wants a stable cell → line mapping without re-wrapping.
pub(crate) fn build_transcript_lines(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
) -> TranscriptLines {
    // This function is the "lossless" transcript flattener:
    // - it asks each cell for its transcript lines (including any per-cell prefixes/indents)
    // - it inserts spacer rows between non-continuation cells to match the viewport layout
    // - it emits parallel metadata so scroll anchoring can map visual lines back to cells.
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut meta: Vec<TranscriptLineMeta> = Vec::new();
    let mut joiner_before: Vec<Option<String>> = Vec::new();
    let mut has_emitted_lines = false;

    for (cell_index, cell) in cells.iter().enumerate() {
        // Cells provide joiners alongside lines so copy can distinguish hard breaks from soft wraps
        // (and preserve the exact whitespace at wrap boundaries).
        let rendered = cell.transcript_lines_with_joiners(width);
        if rendered.lines.is_empty() {
            continue;
        }

        // Cells that are not stream continuations are separated by an explicit spacer row.
        // This keeps the flattened transcript aligned with what the user sees in the viewport
        // and preserves intentional blank lines in copy.
        if !cell.is_stream_continuation() {
            if has_emitted_lines {
                lines.push(Line::from(""));
                meta.push(TranscriptLineMeta::Spacer);
                joiner_before.push(None);
            } else {
                has_emitted_lines = true;
            }
        }

        for (line_in_cell, line) in rendered.lines.into_iter().enumerate() {
            // `line_in_cell` is the *visual* line index within the cell. Consumers use this for
            // anchoring (e.g., "keep this row visible when the transcript reflows").
            meta.push(TranscriptLineMeta::CellLine {
                cell_index,
                line_in_cell,
            });
            lines.push(line);
            // Maintain the `joiner_before` invariant: exactly one entry per output line.
            joiner_before.push(
                rendered
                    .joiner_before
                    .get(line_in_cell)
                    .cloned()
                    .unwrap_or(None),
            );
        }
    }

    TranscriptLines {
        lines,
        meta,
        joiner_before,
    }
}

/// Build flattened transcript lines as they appear in the transcript viewport.
///
/// This applies *viewport wrapping* to prose lines, while deliberately avoiding
/// wrapping for preformatted content (currently detected via the code-block
/// line style) so indentation remains meaningful for copy/paste.
pub(crate) fn build_wrapped_transcript_lines(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
) -> TranscriptLines {
    use crate::render::line_utils::line_to_static;
    use ratatui::style::Color;

    if width == 0 {
        return TranscriptLines {
            lines: Vec::new(),
            meta: Vec::new(),
            joiner_before: Vec::new(),
        };
    }

    let base_opts: crate::wrapping::RtOptions<'_> =
        crate::wrapping::RtOptions::new(width.max(1) as usize);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut meta: Vec<TranscriptLineMeta> = Vec::new();
    let mut joiner_before: Vec<Option<String>> = Vec::new();
    let mut has_emitted_lines = false;

    for (cell_index, cell) in cells.iter().enumerate() {
        // Start from each cell's transcript view (prefixes/indents already applied), then apply
        // viewport wrapping to prose while keeping preformatted content intact.
        let rendered = cell.transcript_lines_with_joiners(width);
        if rendered.lines.is_empty() {
            continue;
        }

        if !cell.is_stream_continuation() {
            if has_emitted_lines {
                lines.push(Line::from(""));
                meta.push(TranscriptLineMeta::Spacer);
                joiner_before.push(None);
            } else {
                has_emitted_lines = true;
            }
        }

        // `visual_line_in_cell` counts the output visual lines produced from this cell *after* any
        // viewport wrapping. This is distinct from `base_idx` (the index into the cell's input
        // lines), since a single input line may wrap into multiple visual lines.
        let mut visual_line_in_cell: usize = 0;
        let mut first = true;
        for (base_idx, base_line) in rendered.lines.iter().enumerate() {
            // Preserve code blocks (and other preformatted text) by not applying
            // viewport wrapping, so indentation remains meaningful for copy/paste.
            if base_line.style.fg == Some(Color::Cyan) {
                lines.push(base_line.clone());
                meta.push(TranscriptLineMeta::CellLine {
                    cell_index,
                    line_in_cell: visual_line_in_cell,
                });
                visual_line_in_cell = visual_line_in_cell.saturating_add(1);
                // Preformatted lines are treated as hard breaks; we keep the cell-provided joiner
                // (which is typically `None`).
                joiner_before.push(
                    rendered
                        .joiner_before
                        .get(base_idx)
                        .cloned()
                        .unwrap_or(None),
                );
                first = false;
                continue;
            }

            let opts = if first {
                base_opts.clone()
            } else {
                // For subsequent input lines within a cell, treat the "initial" indent as the
                // cell's subsequent indent (matches textarea wrapping expectations).
                base_opts
                    .clone()
                    .initial_indent(base_opts.subsequent_indent.clone())
            };
            // `word_wrap_line_with_joiners` returns both the wrapped visual lines and, for each
            // continuation segment, the exact joiner substring that should be inserted instead of a
            // newline when copying as a logical line.
            let (wrapped, wrapped_joiners) =
                crate::wrapping::word_wrap_line_with_joiners(base_line, opts);

            for (seg_idx, (wrapped_line, seg_joiner)) in
                wrapped.into_iter().zip(wrapped_joiners).enumerate()
            {
                lines.push(line_to_static(&wrapped_line));
                meta.push(TranscriptLineMeta::CellLine {
                    cell_index,
                    line_in_cell: visual_line_in_cell,
                });
                visual_line_in_cell = visual_line_in_cell.saturating_add(1);

                if seg_idx == 0 {
                    // The first wrapped segment corresponds to the original input line, so we use
                    // the cell-provided joiner (hard break vs soft break *between input lines*).
                    joiner_before.push(
                        rendered
                            .joiner_before
                            .get(base_idx)
                            .cloned()
                            .unwrap_or(None),
                    );
                } else {
                    // Subsequent wrapped segments are soft-wrap continuations produced by viewport
                    // wrapping, so we use the wrap-derived joiner.
                    joiner_before.push(seg_joiner);
                }
            }

            first = false;
        }
    }

    TranscriptLines {
        lines,
        meta,
        joiner_before,
    }
}

/// Render flattened transcript lines into ANSI strings suitable for printing after the TUI exits.
///
/// This helper mirrors the transcript viewport behavior:
/// - Merges line-level style into each span so ANSI output matches on-screen styling.
/// - For user-authored rows, pads the background style out to the full terminal width so prompts
///   appear as solid blocks in scrollback.
/// - Streams spans through the shared vt100 writer so downstream tests and tools see consistent
///   escape sequences.
pub(crate) fn render_lines_to_ansi(
    lines: &[Line<'static>],
    line_meta: &[TranscriptLineMeta],
    is_user_cell: &[bool],
    width: u16,
) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            // Determine whether this visual line belongs to a user-authored cell. We use this to
            // pad the background to the full terminal width so prompts appear as solid blocks in
            // scrollback.
            let is_user_row = line_meta
                .get(idx)
                .and_then(TranscriptLineMeta::cell_index)
                .map(|cell_index| is_user_cell.get(cell_index).copied().unwrap_or(false))
                .unwrap_or(false);

            // Line-level styles in ratatui apply to the entire line, but spans can also have their
            // own styles. ANSI output is span-based, so we "bake" the line style into every span by
            // patching span style with the line style.
            let mut merged_spans: Vec<ratatui::text::Span<'static>> = line
                .spans
                .iter()
                .map(|span| ratatui::text::Span {
                    style: span.style.patch(line.style),
                    content: span.content.clone(),
                })
                .collect();

            if is_user_row && width > 0 {
                // For user rows, pad out to the full width so the background color extends across
                // the line in terminal scrollback (mirrors the on-screen viewport behavior).
                let text: String = merged_spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect();
                let text_width = UnicodeWidthStr::width(text.as_str());
                let total_width = usize::from(width);
                if text_width < total_width {
                    let pad_len = total_width.saturating_sub(text_width);
                    if pad_len > 0 {
                        let pad_style = crate::style::user_message_style();
                        merged_spans.push(ratatui::text::Span {
                            style: pad_style,
                            content: " ".repeat(pad_len).into(),
                        });
                    }
                }
            }

            let mut buf: Vec<u8> = Vec::new();
            let _ = crate::insert_history::write_spans(&mut buf, merged_spans.iter());
            String::from_utf8(buf).unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::TranscriptLinesWithJoiners;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    #[derive(Debug)]
    struct FakeCell {
        lines: Vec<Line<'static>>,
        joiner_before: Vec<Option<String>>,
        is_stream_continuation: bool,
    }

    impl HistoryCell for FakeCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines_with_joiners(&self, _width: u16) -> TranscriptLinesWithJoiners {
            TranscriptLinesWithJoiners {
                lines: self.lines.clone(),
                joiner_before: self.joiner_before.clone(),
            }
        }

        fn is_stream_continuation(&self) -> bool {
            self.is_stream_continuation
        }
    }

    fn concat_line(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn build_wrapped_transcript_lines_threads_joiners_and_spacers() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(FakeCell {
                lines: vec![Line::from("• hello world")],
                joiner_before: vec![None],
                is_stream_continuation: false,
            }),
            Arc::new(FakeCell {
                lines: vec![Line::from("• foo bar")],
                joiner_before: vec![None],
                is_stream_continuation: false,
            }),
        ];

        // Force wrapping so we get soft-wrap joiners for the second segment of each cell's line.
        let transcript = build_wrapped_transcript_lines(&cells, 8);

        assert_eq!(transcript.lines.len(), transcript.meta.len());
        assert_eq!(transcript.lines.len(), transcript.joiner_before.len());

        let rendered: Vec<String> = transcript.lines.iter().map(concat_line).collect();
        assert_eq!(rendered, vec!["• hello", "world", "", "• foo", "bar"]);

        assert_eq!(
            transcript.meta,
            vec![
                TranscriptLineMeta::CellLine {
                    cell_index: 0,
                    line_in_cell: 0
                },
                TranscriptLineMeta::CellLine {
                    cell_index: 0,
                    line_in_cell: 1
                },
                TranscriptLineMeta::Spacer,
                TranscriptLineMeta::CellLine {
                    cell_index: 1,
                    line_in_cell: 0
                },
                TranscriptLineMeta::CellLine {
                    cell_index: 1,
                    line_in_cell: 1
                },
            ]
        );

        assert_eq!(
            transcript.joiner_before,
            vec![
                None,
                Some(" ".to_string()),
                None,
                None,
                Some(" ".to_string()),
            ]
        );
    }
}
