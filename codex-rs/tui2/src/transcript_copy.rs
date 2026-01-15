//! Converting a transcript selection to clipboard text.
//!
//! Copy is driven by a content-relative selection (`TranscriptSelectionPoint`),
//! but the transcript is rendered with styling and wrapping for the TUI. This
//! module reconstructs clipboard text from the rendered transcript lines while
//! preserving user expectations:
//!
//! - Soft-wrapped prose is treated as a single logical line when copying.
//! - Code blocks preserve meaningful indentation.
//! - Markdown “source markers” are emitted when copying (backticks for inline
//!   code, triple-backtick fences for code blocks) even if the on-screen
//!   rendering is styled differently.
//!
//! ## Inputs and invariants
//!
//! Clipboard reconstruction is performed over the same *visual lines* that are
//! rendered in the transcript viewport:
//!
//! - `lines`: wrapped transcript `Line`s, including the gutter spans.
//! - `joiner_before`: a parallel vector describing which wrapped lines are
//!   *soft wrap* continuations (and what to insert at the wrap boundary).
//! - `(line_index, column)` selection points in *content space* (columns exclude
//!   the gutter).
//!
//! Callers must keep `lines` and `joiner_before` aligned. In practice, `App`
//! obtains both from `transcript_render`, which itself builds from each cell's
//! `HistoryCell::transcript_lines_with_joiners` implementation.
//!
//! ## Style-derived Markdown cues
//!
//! For fidelity, we copy Markdown source markers even though the viewport may
//! render content using styles instead of literal characters. Today, the copy
//! logic derives "inline code" and "code block" boundaries from the styling we
//! apply during rendering (currently cyan spans/lines).
//!
//! If transcript styling changes (for example, if code blocks stop using cyan),
//! update `is_code_block_line` and [`span_is_inline_code`] so clipboard output
//! continues to match user expectations.
//!
//! The caller can choose whether copy covers only the visible viewport range
//! (by passing `visible_start..visible_end`) or spans the entire transcript
//! (by passing `0..lines.len()`).
//!
//! UI affordances (keybinding detection and the on-screen "copy" pill) live in
//! `transcript_copy_ui`.

use ratatui::text::Line;
use ratatui::text::Span;

use crate::history_cell::HistoryCell;
use crate::transcript_selection::TRANSCRIPT_GUTTER_COLS;
use crate::transcript_selection::TranscriptSelection;
use crate::transcript_selection::TranscriptSelectionPoint;
use std::sync::Arc;

/// Render the current transcript selection into clipboard text.
///
/// This is the `App`-level helper: it rebuilds wrapped transcript lines using
/// the same rules as the on-screen viewport and then applies
/// [`selection_to_copy_text`] across the full transcript range (including
/// off-screen lines).
pub(crate) fn selection_to_copy_text_for_cells(
    cells: &[Arc<dyn HistoryCell>],
    selection: TranscriptSelection,
    width: u16,
) -> Option<String> {
    let (anchor, head) = selection.anchor.zip(selection.head)?;

    let transcript = crate::transcript_render::build_wrapped_transcript_lines(cells, width);
    let total_lines = transcript.lines.len();
    if total_lines == 0 {
        return None;
    }

    selection_to_copy_text(
        &transcript.lines,
        &transcript.joiner_before,
        anchor,
        head,
        0,
        total_lines,
        width,
    )
}

/// Render the selected region into clipboard text.
///
/// `lines` must be the wrapped transcript lines as rendered by the TUI,
/// including the leading gutter spans. `start`/`end` columns are expressed in
/// content-space (excluding the gutter), and will be ordered internally if the
/// endpoints are reversed.
///
/// `joiner_before[i]` is the exact string to insert *before* `lines[i]` when
/// it is a continuation of a soft-wrapped prose line. This enables copy to
/// treat soft-wrapped prose as a single logical line.
///
/// Notes:
///
/// - For code/preformatted runs, copy is permitted to extend beyond the
///   viewport width when the user selects “to the right edge”, so we avoid
///   producing truncated logical lines in narrow terminals.
/// - Markdown markers are derived from render-time styles (see module docs).
/// - Column math is display-width-aware (wide glyphs count as multiple columns).
///
/// Returns `None` if the inputs imply an empty selection or if `width` is too
/// small to contain the gutter plus at least one content column.
pub(crate) fn selection_to_copy_text(
    lines: &[Line<'static>],
    joiner_before: &[Option<String>],
    start: TranscriptSelectionPoint,
    end: TranscriptSelectionPoint,
    visible_start: usize,
    visible_end: usize,
    width: u16,
) -> Option<String> {
    use ratatui::style::Color;

    if width <= TRANSCRIPT_GUTTER_COLS {
        return None;
    }

    // Selection points are expressed in content-relative coordinates and may be provided in either
    // direction (dragging "backwards"). Normalize to a forward `(start, end)` pair so the rest of
    // the logic can assume `start <= end`.
    let (start, end) = order_points(start, end);
    if start == end {
        return None;
    }

    // Transcript `Line`s include a left gutter (bullet/prefix space). Selection columns exclude the
    // gutter, so we translate selection columns to absolute columns by adding `base_x`.
    let base_x = TRANSCRIPT_GUTTER_COLS;
    let max_x = width.saturating_sub(1);

    let mut out = String::new();
    let mut prev_selected_line: Option<usize> = None;

    // We emit Markdown fences around runs of code/preformatted visual lines so:
    // - the clipboard captures source-style markers (` ``` `) even if the viewport is stylized
    // - indentation is preserved and paste is stable in editors
    let mut in_code_run = false;

    // `wrote_any` lets us handle separators (newline or soft-wrap joiner) without special-casing
    // "first output line" at every decision point.
    let mut wrote_any = false;

    for line_index in visible_start..visible_end {
        // Only consider lines that intersect the selection's line range. (Selection endpoints are
        // clamped elsewhere; if the indices don't exist, `lines.get(...)` returns `None`.)
        if line_index < start.line_index || line_index > end.line_index {
            continue;
        }

        let line = lines.get(line_index)?;

        // Code blocks (and other preformatted content) are detected via styling and copied as
        // "verbatim lines" (no inline Markdown re-encoding). This also enables special handling for
        // narrow terminals: selecting "to the right edge" should copy the full logical line, not a
        // viewport-truncated slice.
        let is_code_block_line = line.style.fg == Some(Color::Cyan);

        // Flatten the line to compute the rightmost non-space column. We use that to:
        // - avoid copying trailing right-margin padding
        // - clamp prose selection to the viewport width
        let flat = line_to_flat(line);
        let text_end = if is_code_block_line {
            last_non_space_col(flat.as_str())
        } else {
            last_non_space_col(flat.as_str()).map(|c| c.min(max_x))
        };

        // Convert selection endpoints into a selection range for this specific visual line:
        // - first line clamps the start column
        // - last line clamps the end column
        // - intermediate lines select the full line.
        let line_start_col = if line_index == start.line_index {
            start.column
        } else {
            0
        };
        let line_end_col = if line_index == end.line_index {
            end.column
        } else {
            max_x.saturating_sub(base_x)
        };

        let row_sel_start = base_x.saturating_add(line_start_col).min(max_x);

        // For code/preformatted lines, treat "selection ends at the viewport edge" as a special
        // "copy to end of logical line" case. This prevents narrow terminals from producing
        // truncated clipboard content when the user drags to the right edge.
        let row_sel_end = if is_code_block_line && line_end_col >= max_x.saturating_sub(base_x) {
            u16::MAX
        } else {
            base_x.saturating_add(line_end_col).min(max_x)
        };
        if row_sel_start > row_sel_end {
            continue;
        }

        let selected_line = if let Some(text_end) = text_end {
            let from_col = row_sel_start.max(base_x);
            let to_col = row_sel_end.min(text_end);
            if from_col > to_col {
                Line::default().style(line.style)
            } else {
                slice_line_by_cols(line, from_col, to_col)
            }
        } else {
            Line::default().style(line.style)
        };

        // Convert the selected `Line` into Markdown source:
        // - For prose: wrap inline-code spans in backticks.
        // - For code blocks: return the raw flat text so we preserve indentation/spacing.
        let line_text = line_to_markdown(&selected_line, is_code_block_line);

        // Track transitions into/out of code/preformatted runs and emit triple-backtick fences.
        // We always separate a code run from prior prose with a newline.
        if is_code_block_line && !in_code_run {
            if wrote_any {
                out.push('\n');
            }
            out.push_str("```");
            out.push('\n');
            in_code_run = true;
            prev_selected_line = None;
            wrote_any = true;
        } else if !is_code_block_line && in_code_run {
            out.push('\n');
            out.push_str("```");
            out.push('\n');
            in_code_run = false;
            prev_selected_line = None;
            wrote_any = true;
        }

        // When copying inside a code run, every selected visual line becomes a literal line inside
        // the fence (no soft-wrap joining). We preserve explicit blank lines by writing empty
        // strings as a line.
        if in_code_run {
            if wrote_any && (!out.ends_with('\n') || prev_selected_line.is_some()) {
                out.push('\n');
            }
            out.push_str(line_text.as_str());
            prev_selected_line = Some(line_index);
            wrote_any = true;
            continue;
        }

        // Prose path:
        // - If this line is a soft-wrap continuation of the previous selected line, insert the
        //   recorded joiner (often spaces) instead of a newline.
        // - Otherwise, insert a newline to preserve hard breaks.
        if wrote_any {
            let joiner = joiner_before.get(line_index).cloned().unwrap_or(None);
            if prev_selected_line == Some(line_index.saturating_sub(1))
                && let Some(joiner) = joiner
            {
                out.push_str(joiner.as_str());
            } else {
                out.push('\n');
            }
        }

        out.push_str(line_text.as_str());
        prev_selected_line = Some(line_index);
        wrote_any = true;
    }

    if in_code_run {
        out.push('\n');
        out.push_str("```");
    }

    (!out.is_empty()).then_some(out)
}

/// Order two selection endpoints into `(start, end)` in transcript order.
///
/// Dragging can produce reversed endpoints; callers typically want a normalized range before
/// iterating visual lines.
fn order_points(
    a: TranscriptSelectionPoint,
    b: TranscriptSelectionPoint,
) -> (TranscriptSelectionPoint, TranscriptSelectionPoint) {
    if (b.line_index < a.line_index) || (b.line_index == a.line_index && b.column < a.column) {
        (b, a)
    } else {
        (a, b)
    }
}

/// Flatten a styled `Line` into its plain text content.
///
/// This is used for cursor/column arithmetic and for emitting plain-text code lines.
fn line_to_flat(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
}

/// Return the last non-space *display column* in `flat` (inclusive).
///
/// This is display-width-aware, so wide glyphs (e.g. CJK) advance by more than one column.
///
/// Rationale: transcript rendering often pads out to the viewport width; copy should avoid
/// including that right-margin whitespace.
fn last_non_space_col(flat: &str) -> Option<u16> {
    use unicode_width::UnicodeWidthChar;

    let mut col: u16 = 0;
    let mut last: Option<u16> = None;
    for ch in flat.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if ch != ' ' {
            let end = col.saturating_add(w.saturating_sub(1));
            last = Some(end);
        }
        col = col.saturating_add(w);
    }
    last
}

/// Map a display-column range to a UTF-8 byte range within `flat`.
///
/// The returned range is suitable for slicing `flat` and for slicing the original `Span` strings
/// (once translated into span-local offsets).
///
/// This walks Unicode scalar values and advances by display width so callers can slice based on the
/// same column semantics the selection model uses.
fn byte_range_for_cols(flat: &str, start_col: u16, end_col: u16) -> Option<std::ops::Range<usize>> {
    use unicode_width::UnicodeWidthChar;

    // We translate selection columns (display columns, not bytes) into a UTF-8 byte range. This is
    // intentionally Unicode-width aware: wide glyphs cover multiple columns but occupy one `char`
    // and several bytes.
    //
    // Strategy:
    // - Walk `flat` by `char_indices()` while tracking the current display column.
    // - The start byte is the first char whose rendered columns intersect `start_col`.
    // - The end byte is the end of the last char whose rendered columns intersect `end_col`.
    let mut col: u16 = 0;
    let mut start_byte: Option<usize> = None;
    let mut end_byte: Option<usize> = None;

    for (idx, ch) in flat.char_indices() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        let end = col.saturating_add(w.saturating_sub(1));

        // Start is inclusive: select the first glyph whose right edge reaches the start column.
        if start_byte.is_none() && end >= start_col {
            start_byte = Some(idx);
        }

        // End is inclusive in column space; keep extending end byte while we're still at/before
        // `end_col`. This includes a wide glyph even if it starts before `end_col` but ends after.
        if col <= end_col {
            end_byte = Some(idx + ch.len_utf8());
        }

        col = col.saturating_add(w);
        if col > end_col && start_byte.is_some() {
            break;
        }
    }

    match (start_byte, end_byte) {
        (Some(s), Some(e)) if e >= s => Some(s..e),
        _ => None,
    }
}

/// Slice a styled `Line` by display columns, preserving per-span style.
///
/// This is the core "selection → styled substring" helper used before Markdown re-encoding. It
/// avoids mixing styles across spans by slicing each contributing span independently, then
/// reassembling them into a new `Line` with the original line-level style.
fn slice_line_by_cols(line: &Line<'static>, start_col: u16, end_col: u16) -> Line<'static> {
    // `Line` spans store independent string slices with their own styles. To slice by columns while
    // preserving styling, we:
    // 1) Flatten the line and compute the desired UTF-8 byte range in the flattened string.
    // 2) Compute each span's byte range within the flattened string.
    // 3) Intersect the selection range with each span range and slice per-span, preserving styles.
    let flat = line_to_flat(line);
    let mut span_bounds: Vec<(std::ops::Range<usize>, ratatui::style::Style)> = Vec::new();
    let mut acc = 0usize;
    for s in &line.spans {
        let start = acc;
        let text = s.content.as_ref();
        acc += text.len();
        span_bounds.push((start..acc, s.style));
    }

    let Some(range) = byte_range_for_cols(flat.as_str(), start_col, end_col) else {
        return Line::default().style(line.style);
    };

    // Translate the flattened byte range back into (span-local) slices.
    let start_byte = range.start;
    let end_byte = range.end;
    let mut spans: Vec<ratatui::text::Span<'static>> = Vec::new();
    for (i, (r, style)) in span_bounds.iter().enumerate() {
        let s = r.start;
        let e = r.end;
        if e <= start_byte {
            continue;
        }
        if s >= end_byte {
            break;
        }
        let seg_start = start_byte.max(s);
        let seg_end = end_byte.min(e);
        if seg_end > seg_start {
            let local_start = seg_start - s;
            let local_end = seg_end - s;
            let content = line.spans[i].content.as_ref();
            spans.push(ratatui::text::Span {
                style: *style,
                content: content[local_start..local_end].to_string().into(),
            });
        }
        if e >= end_byte {
            break;
        }
    }
    Line::from(spans).style(line.style)
}

/// Whether a span should be treated as "inline code" when reconstructing Markdown.
///
/// TUI2 renders inline code using a cyan foreground. Links also use cyan, but are underlined, so we
/// exclude underlined cyan spans to avoid wrapping links in backticks.
fn span_is_inline_code(span: &Span<'_>) -> bool {
    use ratatui::style::Color;

    span.style.fg == Some(Color::Cyan)
        && !span
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED)
}

/// Convert a selected, styled `Line` back into Markdown-ish source text.
///
/// - For prose: wraps runs of inline-code spans in backticks to preserve the source marker.
/// - For code blocks: emits the raw flat text (no additional escaping), since the entire run will
///   be wrapped in triple-backtick fences by the caller.
fn line_to_markdown(line: &Line<'static>, is_code_block: bool) -> String {
    if is_code_block {
        return line_to_flat(line);
    }

    let mut out = String::new();
    let mut in_code = false;
    for span in &line.spans {
        let is_code = span_is_inline_code(span);
        if is_code && !in_code {
            out.push('`');
            in_code = true;
        } else if !is_code && in_code {
            out.push('`');
            in_code = false;
        }
        out.push_str(span.content.as_ref());
    }
    if in_code {
        out.push('`');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Style;
    use ratatui::style::Stylize;

    #[test]
    fn selection_to_copy_text_returns_none_for_zero_content_width() {
        let lines = vec![Line::from("• Hello")];
        let joiner_before = vec![None];
        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: 1,
        };

        assert_eq!(
            selection_to_copy_text(
                &lines,
                &joiner_before,
                start,
                end,
                0,
                lines.len(),
                TRANSCRIPT_GUTTER_COLS,
            ),
            None
        );
    }

    #[test]
    fn selection_to_copy_text_returns_none_for_empty_selection_point() {
        let lines = vec![Line::from("• Hello")];
        let joiner_before = vec![None];
        let pt = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };

        assert_eq!(
            selection_to_copy_text(&lines, &joiner_before, pt, pt, 0, lines.len(), 20),
            None
        );
    }

    #[test]
    fn selection_to_copy_text_orders_reversed_endpoints() {
        let lines = vec![Line::from("• Hello world")];
        let joiner_before = vec![None];

        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 10,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: 6,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, 1, 80)
            .expect("expected text");

        assert_eq!(out, "world");
    }

    #[test]
    fn copy_selection_soft_wrap_joins_without_newline() {
        let lines = vec![Line::from("• Hello"), Line::from("  world")];
        let joiner_before = vec![None, Some(" ".to_string())];
        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 1,
            column: 100,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, lines.len(), 20)
            .expect("expected text");

        assert_eq!(out, "Hello world");
    }

    #[test]
    fn copy_selection_wraps_inline_code_in_backticks() {
        let lines = vec![Line::from(vec![
            "• ".into(),
            "Use ".into(),
            ratatui::text::Span::from("foo()").style(Style::new().fg(Color::Cyan)),
            " now".into(),
        ])];
        let joiner_before = vec![None];
        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: 100,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, 1, 80)
            .expect("expected text");

        assert_eq!(out, "Use `foo()` now");
    }

    #[test]
    fn selection_to_copy_text_for_cells_reconstructs_full_code_line_beyond_viewport() {
        #[derive(Debug)]
        struct FakeCell {
            lines: Vec<Line<'static>>,
            joiner_before: Vec<Option<String>>,
        }

        impl HistoryCell for FakeCell {
            fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
                self.lines.clone()
            }

            fn transcript_lines_with_joiners(
                &self,
                _width: u16,
            ) -> crate::history_cell::TranscriptLinesWithJoiners {
                crate::history_cell::TranscriptLinesWithJoiners {
                    lines: self.lines.clone(),
                    joiner_before: self.joiner_before.clone(),
                }
            }
        }

        let style = Style::new().fg(Color::Cyan);
        let cell = FakeCell {
            lines: vec![Line::from("•     0123456789ABCDEFGHIJ").style(style)],
            joiner_before: vec![None],
        };
        let cells: Vec<std::sync::Arc<dyn HistoryCell>> = vec![std::sync::Arc::new(cell)];

        let width: u16 = 12;
        let max_x = width.saturating_sub(1);
        let viewport_edge_col = max_x.saturating_sub(TRANSCRIPT_GUTTER_COLS);

        let selection = TranscriptSelection {
            anchor: Some(TranscriptSelectionPoint::new(0, 0)),
            head: Some(TranscriptSelectionPoint::new(0, viewport_edge_col)),
        };

        let out =
            selection_to_copy_text_for_cells(&cells, selection, width).expect("expected text");
        assert_eq!(out, "```\n    0123456789ABCDEFGHIJ\n```");
    }

    #[test]
    fn order_points_orders_by_line_then_column() {
        let a = TranscriptSelectionPoint::new(2, 5);
        let b = TranscriptSelectionPoint::new(1, 10);
        assert_eq!(order_points(a, b), (b, a));

        let a = TranscriptSelectionPoint::new(1, 5);
        let b = TranscriptSelectionPoint::new(1, 10);
        assert_eq!(order_points(a, b), (a, b));
    }

    #[test]
    fn line_to_flat_concatenates_spans() {
        let line = Line::from(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(line_to_flat(&line), "abc");
    }

    #[test]
    fn last_non_space_col_counts_display_width() {
        // "コ" is width 2, so "コX" occupies columns 0..=2.
        assert_eq!(last_non_space_col("コX"), Some(2));
        assert_eq!(last_non_space_col("a  "), Some(0));
        assert_eq!(last_non_space_col("   "), None);
    }

    #[test]
    fn byte_range_for_cols_maps_columns_to_utf8_bytes() {
        let flat = "abcd";
        let range = byte_range_for_cols(flat, 1, 2).expect("range");
        assert_eq!(&flat[range], "bc");

        let flat = "コX";
        let range = byte_range_for_cols(flat, 0, 2).expect("range");
        assert_eq!(&flat[range], "コX");
    }

    #[test]
    fn slice_line_by_cols_preserves_span_styles() {
        let line = Line::from(vec![
            "• ".into(),
            "Hello".red(),
            " ".into(),
            "world".green(),
        ]);

        // Slice "llo wo" (crosses span boundaries).
        let sliced = slice_line_by_cols(&line, 4, 9);
        assert_eq!(line_to_flat(&sliced), "llo wo");
        assert_eq!(sliced.spans.len(), 3);
        assert_eq!(sliced.spans[0].content.as_ref(), "llo");
        assert_eq!(sliced.spans[0].style.fg, Some(Color::Red));
        assert_eq!(sliced.spans[1].content.as_ref(), " ");
        assert_eq!(sliced.spans[2].content.as_ref(), "wo");
        assert_eq!(sliced.spans[2].style.fg, Some(Color::Green));
    }

    #[test]
    fn span_is_inline_code_excludes_underlined_cyan() {
        let inline_code = Span::from("x").style(Style::new().fg(Color::Cyan));
        assert!(span_is_inline_code(&inline_code));

        let link_like = Span::from("x").style(Style::new().fg(Color::Cyan).underlined());
        assert!(!span_is_inline_code(&link_like));

        let other = Span::from("x").style(Style::new().fg(Color::Green));
        assert!(!span_is_inline_code(&other));
    }

    #[test]
    fn line_to_markdown_wraps_contiguous_inline_code_spans() {
        let line = Line::from(vec![
            "Use ".into(),
            Span::from("foo").style(Style::new().fg(Color::Cyan)),
            Span::from("()").style(Style::new().fg(Color::Cyan)),
            " now".into(),
        ]);
        assert_eq!(line_to_markdown(&line, false), "Use `foo()` now");
    }

    #[test]
    fn copy_selection_preserves_wide_glyphs() {
        let lines = vec![Line::from("• コX")];
        let joiner_before = vec![None];

        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: 2,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, 1, 80)
            .expect("expected text");

        assert_eq!(out, "コX");
    }

    #[test]
    fn copy_selection_wraps_code_block_in_fences_and_preserves_indent() {
        let style = Style::new().fg(Color::Cyan);
        let lines = vec![
            Line::from("•     fn main() {}").style(style),
            Line::from("      println!(\"hi\");").style(style),
        ];
        let joiner_before = vec![None, None];
        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 1,
            column: 100,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, lines.len(), 80)
            .expect("expected text");

        assert_eq!(out, "```\n    fn main() {}\n    println!(\"hi\");\n```");
    }

    #[test]
    fn copy_selection_code_block_end_col_at_viewport_edge_copies_full_line() {
        let style = Style::new().fg(Color::Cyan);
        let lines = vec![Line::from("•     0123456789ABCDEFGHIJ").style(style)];
        let joiner_before = vec![None];

        let width: u16 = 12;
        let max_x = width.saturating_sub(1);
        let viewport_edge_col = max_x.saturating_sub(TRANSCRIPT_GUTTER_COLS);

        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: viewport_edge_col,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, 1, width)
            .expect("expected text");

        assert_eq!(out, "```\n    0123456789ABCDEFGHIJ\n```");
    }

    #[test]
    fn copy_selection_code_block_end_col_before_viewport_edge_copies_partial_line() {
        let style = Style::new().fg(Color::Cyan);
        let lines = vec![Line::from("•     0123456789ABCDEFGHIJ").style(style)];
        let joiner_before = vec![None];

        let width: u16 = 12;

        let start = TranscriptSelectionPoint {
            line_index: 0,
            column: 0,
        };
        let end = TranscriptSelectionPoint {
            line_index: 0,
            column: 7,
        };

        let out = selection_to_copy_text(&lines, &joiner_before, start, end, 0, 1, width)
            .expect("expected text");

        assert_eq!(out, "```\n    0123\n```");
    }
}
