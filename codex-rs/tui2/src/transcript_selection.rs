//! Transcript selection helpers.
//!
//! This module defines a content-relative selection model for the inline chat
//! transcript and utilities for extracting the selected region as plain text.
//! Selection endpoints are expressed in terms of flattened, wrapped transcript
//! line indices and columns so they remain stable across scrolling and
//! reflowing when the terminal is resized.
//!
//! Copy uses offscreen rendering into a 1-row `ratatui::Buffer` per visual line
//! to match on-screen glyph layout (including indentation/prefixes) while
//! skipping the transcript gutter.

use itertools::Itertools as _;
use ratatui::prelude::*;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

/// Number of columns reserved for the transcript gutter before the copyable
/// transcript text begins.
pub(crate) const TRANSCRIPT_GUTTER_COLS: u16 = 2;

/// Content-relative selection within the inline transcript viewport.
///
/// Selection endpoints are expressed in terms of flattened, wrapped transcript
/// line indices and columns, so the highlight tracks logical conversation
/// content even when the viewport scrolls or the terminal is resized.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TranscriptSelection {
    /// The selection anchor (fixed start) in transcript coordinates.
    pub(crate) anchor: Option<TranscriptSelectionPoint>,
    /// The selection head (moving end) in transcript coordinates.
    pub(crate) head: Option<TranscriptSelectionPoint>,
}

impl TranscriptSelection {
    /// Create an active selection with both endpoints set.
    pub(crate) fn new(
        anchor: impl Into<TranscriptSelectionPoint>,
        head: impl Into<TranscriptSelectionPoint>,
    ) -> Self {
        Self {
            anchor: Some(anchor.into()),
            head: Some(head.into()),
        }
    }
}

/// A single endpoint of a transcript selection.
///
/// `line_index` is an index into the flattened wrapped transcript lines, and
/// `column` is a zero-based column offset within that visual line, counted from
/// the first content column to the right of the transcript gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TranscriptSelectionPoint {
    /// Index into the flattened wrapped transcript lines.
    pub(crate) line_index: usize,
    /// Zero-based column offset within the wrapped line, relative to the first
    /// content column to the right of the transcript gutter.
    pub(crate) column: u16,
}

impl TranscriptSelectionPoint {
    /// Create a selection endpoint at a given wrapped line index and column.
    pub(crate) const fn new(line_index: usize, column: u16) -> Self {
        Self { line_index, column }
    }
}

impl From<(usize, u16)> for TranscriptSelectionPoint {
    /// Convert from `(line_index, column)`.
    fn from((line_index, column): (usize, u16)) -> Self {
        Self::new(line_index, column)
    }
}

/// Extract the full transcript selection as plain text.
///
/// This intentionally does *not* use viewport state. Instead it:
///
/// - Applies the same word-wrapping used for on-screen rendering, producing
///   flattened "visual" lines.
/// - Renders each selected visual line into a 1-row offscreen `Buffer` and
///   extracts the selected character cells from that buffer.
///
/// Using the rendered buffer (instead of slicing the source strings) keeps copy
/// semantics aligned with what the user sees on screen, including:
///
/// - Prefixes / indentation introduced during rendering (e.g. list markers).
/// - The transcript gutter: selection columns are defined relative to the
///   first content column to the right of the gutter (`base_x =
///   TRANSCRIPT_GUTTER_COLS`).
/// - Multi-cell glyph rendering decisions made by the backend.
///
/// Notes:
///
/// - Trailing padding to the right margin is not included; we clamp each line
///   to the last non-space glyph to avoid copying a full-width block of spaces.
/// - `TranscriptSelectionPoint::column` can be arbitrarily large (e.g.
///   `u16::MAX` when dragging to the right edge); we clamp to the rendered line
///   width so "copy to end of line" behaves naturally.
pub(crate) fn selection_text(
    lines: &[Line<'static>],
    selection: TranscriptSelection,
    width: u16,
) -> Option<String> {
    let (anchor, head) = selection.anchor.zip(selection.head)?;
    if anchor == head {
        return None;
    }

    let (start, end) = ordered_endpoints(anchor, head);
    let wrapped = wrap_transcript_lines(lines, width)?;
    let ctx = RenderContext::new(width)?;

    let total_lines = wrapped.len();
    if start.line_index >= total_lines {
        return None;
    }

    // If the selection ends beyond the last wrapped line, clamp it so selection
    // behaves like "copy through the end" rather than returning no text.
    let (end_line_index, end_is_clamped) = clamp_end_line(end.line_index, total_lines)?;

    let mut buf = Buffer::empty(ctx.area);
    let mut lines_out: Vec<String> = Vec::new();

    for (line_index, line) in wrapped
        .iter()
        .enumerate()
        .take(end_line_index + 1)
        .skip(start.line_index)
    {
        buf.reset();
        line.render_ref(ctx.area, &mut buf);

        let Some((row_sel_start, row_sel_end)) =
            ctx.selection_bounds_for_line(line_index, start, end, end_is_clamped)
        else {
            // Preserve row count/newlines within the selection even if this
            // particular visual line produces no selected cells.
            lines_out.push(String::new());
            continue;
        };

        let Some(content_end_x) = ctx.content_end_x(&buf) else {
            // Preserve explicit blank lines (e.g., spacer rows) in the selection.
            lines_out.push(String::new());
            continue;
        };

        let from_x = row_sel_start.max(ctx.base_x);
        let to_x = row_sel_end.min(content_end_x);
        if from_x > to_x {
            // Preserve row count/newlines even when selection falls beyond the
            // rendered content for this visual line.
            lines_out.push(String::new());
            continue;
        }

        lines_out.push(ctx.extract_text(&buf, from_x, to_x));
    }

    Some(lines_out.join("\n"))
}

/// Return `(start, end)` with `start <= end` in transcript order.
pub(crate) fn ordered_endpoints(
    anchor: TranscriptSelectionPoint,
    head: TranscriptSelectionPoint,
) -> (TranscriptSelectionPoint, TranscriptSelectionPoint) {
    if anchor <= head {
        (anchor, head)
    } else {
        (head, anchor)
    }
}

/// Wrap transcript lines using the same algorithm as on-screen rendering.
///
/// Returns `None` for invalid widths or when wrapping produces no visual lines.
fn wrap_transcript_lines<'a>(lines: &'a [Line<'static>], width: u16) -> Option<Vec<Line<'a>>> {
    if width == 0 || lines.is_empty() {
        return None;
    }

    let wrapped = crate::wrapping::word_wrap_lines_borrowed(lines, width.max(1) as usize);
    (!wrapped.is_empty()).then_some(wrapped)
}

/// Context for rendering a single wrapped transcript line into a 1-row buffer and
/// extracting selected cells.
#[derive(Debug, Clone, Copy)]
struct RenderContext {
    /// One-row region used for offscreen rendering.
    area: Rect,
    /// X coordinate where copyable transcript content begins (gutter skipped).
    base_x: u16,
    /// Maximum X coordinate inside the render area (inclusive).
    max_x: u16,
    /// Maximum content-relative column (0-based) within the render area.
    max_content_col: u16,
}

impl RenderContext {
    /// Create a 1-row render context for a given terminal width.
    ///
    /// Returns `None` when the width is too small to hold any copyable content
    /// (e.g. the gutter consumes the entire row).
    fn new(width: u16) -> Option<Self> {
        if width == 0 {
            return None;
        }

        let area = Rect::new(0, 0, width, 1);
        let base_x = area.x.saturating_add(TRANSCRIPT_GUTTER_COLS);
        let max_x = area.right().saturating_sub(1);
        if base_x > max_x {
            return None;
        }

        Some(Self {
            area,
            base_x,
            max_x,
            max_content_col: max_x.saturating_sub(base_x),
        })
    }

    /// Compute the inclusive selection X range for this visual line.
    ///
    /// `start`/`end` columns are content-relative (0 starts at the first column
    /// to the right of the transcript gutter). For the terminal line containing
    /// the selection endpoint, this clamps the selection to that endpoint; for
    /// intermediate lines it selects the whole line.
    ///
    /// If the selection end was clamped to the last available line (meaning the
    /// logical selection extended beyond the rendered transcript), the final
    /// line is treated as selecting through the end of that line.
    fn selection_bounds_for_line(
        &self,
        line_index: usize,
        start: TranscriptSelectionPoint,
        end: TranscriptSelectionPoint,
        end_is_clamped: bool,
    ) -> Option<(u16, u16)> {
        let line_start_col = if line_index == start.line_index {
            start.column
        } else {
            0
        };
        let line_end_col = if !end_is_clamped && line_index == end.line_index {
            end.column
        } else {
            self.max_content_col
        };

        let row_sel_start = self.base_x.saturating_add(line_start_col);
        let row_sel_end = self.base_x.saturating_add(line_end_col).min(self.max_x);

        (row_sel_start <= row_sel_end).then_some((row_sel_start, row_sel_end))
    }

    /// Find the last non-space glyph in the rendered content area.
    ///
    /// This is used to avoid copying right-margin padding when the rendered row
    /// is shorter than the terminal width.
    fn content_end_x(&self, buf: &Buffer) -> Option<u16> {
        (self.base_x..=self.max_x)
            .rev()
            .find(|&x| buf[(x, 0)].symbol() != " ")
    }

    /// Extract rendered cell contents from an inclusive `[from_x, to_x]` range.
    ///
    /// Note: terminals represent wide glyphs (e.g. CJK characters) using multiple
    /// cells, but only the first cell contains the glyph's symbol. The remaining
    /// cells are "continuation" cells that should not be copied as separate
    /// characters. Ratatui marks those continuation cells as a single space in
    /// the buffer, so we must explicitly skip `width - 1` following cells after
    /// reading each rendered symbol to avoid producing output like `"コ X"`.
    fn extract_text(&self, buf: &Buffer, from_x: u16, to_x: u16) -> String {
        (from_x..=to_x)
            .batching(|xs| {
                let x = xs.next()?;
                let symbol = buf[(x, 0)].symbol();
                for _ in 0..symbol.width().saturating_sub(1) {
                    xs.next();
                }
                (!symbol.is_empty()).then_some(symbol)
            })
            .join("")
    }
}

/// Clamp `end_line_index` to the last available line and report if it was clamped.
///
/// Returns `None` when there are no wrapped lines.
fn clamp_end_line(end_line_index: usize, total_lines: usize) -> Option<(usize, bool)> {
    if total_lines == 0 {
        return None;
    }

    let clamped = end_line_index.min(total_lines.saturating_sub(1));
    Some((clamped, clamped != end_line_index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn selection_text_returns_none_when_missing_endpoints() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello".into()])];
        let selection = TranscriptSelection::default();

        assert_eq!(selection_text(&lines, selection, 40), None);
    }

    #[test]
    fn selection_text_returns_none_when_endpoints_equal() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello".into()])];
        let selection = TranscriptSelection::new((0, 2), (0, 2));

        assert_eq!(selection_text(&lines, selection, 40), None);
    }

    #[test]
    fn selection_text_returns_none_for_empty_lines() {
        let selection = TranscriptSelection::new((0, 0), (0, 1));

        assert_eq!(selection_text(&[], selection, 40), None);
    }

    #[test]
    fn selection_text_returns_none_for_zero_width() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 1));

        assert_eq!(selection_text(&lines, selection, 0), None);
    }

    #[test]
    fn selection_text_returns_none_when_width_smaller_than_gutter() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 1));

        assert_eq!(selection_text(&lines, selection, 2), None);
    }

    #[test]
    fn selection_text_skips_gutter_prefix() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 4));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "Hello");
    }

    #[test]
    fn selection_text_selects_substring_single_line() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello world".into()])];
        let selection = TranscriptSelection::new((0, 6), (0, 10));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "world");
    }

    #[test]
    fn selection_text_preserves_interior_spaces() {
        let lines = vec![Line::from(vec!["• ".into(), "a  b".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 3));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "a  b");
    }

    #[test]
    fn selection_text_skips_hidden_wide_glyph_cells() {
        let lines = vec![Line::from(vec!["• ".into(), "コX".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 2));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "コX");
    }

    #[test]
    fn selection_text_orders_reversed_endpoints() {
        let lines = vec![Line::from(vec!["• ".into(), "Hello world".into()])];
        let selection = TranscriptSelection::new((0, 10), (0, 6));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "world");
    }

    #[test]
    fn selection_text_selects_multiple_lines_with_partial_endpoints() {
        let lines = vec![
            Line::from(vec!["• ".into(), "abcde".into()]),
            Line::from(vec!["• ".into(), "fghij".into()]),
            Line::from(vec!["• ".into(), "klmno".into()]),
        ];
        let selection = TranscriptSelection::new((0, 2), (2, 2));

        assert_eq!(
            selection_text(&lines, selection, 40).unwrap(),
            "cde\nfghij\nklm"
        );
    }

    #[test]
    fn selection_text_selects_to_end_of_line_for_large_column() {
        let lines = vec![Line::from(vec!["• ".into(), "one".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, u16::MAX));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "one");
    }

    #[test]
    fn selection_text_includes_indentation_spaces() {
        let lines = vec![Line::from(vec!["• ".into(), "  ind".into()])];
        let selection = TranscriptSelection::new((0, 0), (0, 4));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "  ind");
    }

    #[test]
    fn selection_text_preserves_empty_lines() {
        let lines = vec![
            Line::from(vec!["• ".into(), "one".into()]),
            Line::from("• "),
            Line::from(vec!["• ".into(), "two".into()]),
        ];
        let selection = TranscriptSelection::new((0, 0), (2, 2));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "one\n\ntwo");
    }

    #[test]
    fn selection_text_clamps_end_line_index() {
        let lines = vec![
            Line::from(vec!["• ".into(), "one".into()]),
            Line::from(vec!["• ".into(), "two".into()]),
        ];
        let selection = TranscriptSelection::new((0, 0), (100, u16::MAX));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "one\ntwo");
    }

    #[test]
    fn selection_text_clamps_end_line_index_ignoring_end_column() {
        let lines = vec![
            Line::from(vec!["• ".into(), "one".into()]),
            Line::from(vec!["• ".into(), "two".into()]),
        ];
        let selection = TranscriptSelection::new((0, 0), (100, 0));

        assert_eq!(selection_text(&lines, selection, 40).unwrap(), "one\ntwo");
    }

    #[test]
    fn selection_text_returns_none_when_start_line_out_of_range() {
        let lines = vec![Line::from(vec!["• ".into(), "one".into()])];
        let selection = TranscriptSelection::new((100, 0), (101, 0));

        assert_eq!(selection_text(&lines, selection, 40), None);
    }
}
