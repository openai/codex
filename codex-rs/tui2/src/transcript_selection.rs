//! Transcript selection helpers.
//!
//! This module owns the inline transcript's selection model and helper
//! utilities:
//!
//! - A **content-relative** selection representation ([`TranscriptSelection`])
//!   expressed in terms of flattened, wrapped transcript line indices and
//!   columns.
//! - A small mouse-driven **selection state machine** (`on_mouse_*` helpers)
//!   that implements "start selection on drag" semantics.
//! - Copy extraction ([`selection_text`]) that matches on-screen glyph layout by
//!   rendering selected lines into an offscreen [`ratatui::Buffer`].
//!
//! ## Coordinate model
//!
//! Selection endpoints are expressed in *wrapped* coordinates so they remain
//! stable across scrolling and reflowing when the terminal is resized:
//!
//! - `line_index` is an index into flattened wrapped transcript lines
//!   (i.e. "visual lines").
//! - `column` is a 0-based column offset within that visual line, measured from
//!   the first content column to the right of the transcript gutter.
//!
//! The transcript gutter is reserved for UI affordances (bullets, prefixes,
//! etc.). The gutter itself is not copyable; both selection highlighting and
//! copy extraction treat selection columns as starting at `base_x =
//! TRANSCRIPT_GUTTER_COLS`.
//!
//! ## Mouse selection semantics
//!
//! The transcript supports click-and-drag selection for copying text. To avoid
//! distracting "1-cell selections" on a simple click, the selection highlight
//! only becomes active once the user drags:
//!
//! - `on_mouse_down`: stores an **anchor** point and clears any existing head.
//! - `on_mouse_drag`: sets the **head** point, creating an active selection
//!   (`anchor` + `head`).
//! - `on_mouse_up`: clears the selection if it never became active (no head) or
//!   if the drag ended at the anchor.
//!
//! The helper APIs return whether the selection state changed so callers can
//! schedule a redraw. `on_mouse_drag` also returns whether the caller should
//! lock the transcript scroll position when dragging while following the bottom
//! during streaming output.

use crate::tui::scrolling::TranscriptScroll;
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TranscriptSelection {
    /// The selection anchor (fixed start) in transcript coordinates.
    pub(crate) anchor: Option<TranscriptSelectionPoint>,
    /// The selection head (moving end) in transcript coordinates.
    pub(crate) head: Option<TranscriptSelectionPoint>,
}

impl TranscriptSelection {
    /// Create an active selection with both endpoints set.
    #[cfg(test)]
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

/// Begin a potential transcript selection (left button down).
///
/// This records an anchor point and clears any existing head. The selection is
/// not considered "active" until a drag sets a head, which avoids highlighting
/// a 1-cell region on simple click.
///
/// Returns whether the selection changed (useful to decide whether to request a
/// redraw).
pub(crate) fn on_mouse_down(
    selection: &mut TranscriptSelection,
    point: Option<TranscriptSelectionPoint>,
) -> bool {
    let before = *selection;
    let Some(point) = point else {
        return false;
    };
    begin(selection, point);
    *selection != before
}

/// The outcome of a mouse drag update.
///
/// This is returned by [`on_mouse_drag`]. It separates selection state updates
/// from `App`-level actions, so callers can decide when to schedule redraws or
/// lock the transcript scroll position.
///
/// `lock_scroll` indicates the caller should lock the transcript viewport (if
/// currently following the bottom) so ongoing streaming output does not move
/// the selection under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseDragOutcome {
    /// Whether the selection changed (useful to decide whether to request a
    /// redraw).
    pub(crate) changed: bool,
    /// Whether the caller should lock the transcript scroll position.
    pub(crate) lock_scroll: bool,
}

/// Update the selection state for a left-button drag.
///
/// This sets the selection head (creating an active selection) and returns:
///
/// - `changed`: whether the selection state changed (useful to decide whether to
///   request a redraw).
/// - `lock_scroll`: whether the caller should lock transcript scrolling to
///   freeze the viewport under the selection while streaming output arrives.
///
/// `point` is expected to already be clamped to the transcript's content area
/// (e.g. not in the gutter). If `point` is `None`, this is a no-op.
pub(crate) fn on_mouse_drag(
    selection: &mut TranscriptSelection,
    scroll: &TranscriptScroll,
    point: Option<TranscriptSelectionPoint>,
    streaming: bool,
) -> MouseDragOutcome {
    let before = *selection;
    let Some(point) = point else {
        return MouseDragOutcome {
            changed: false,
            lock_scroll: false,
        };
    };
    let lock_scroll = drag(selection, scroll, point, streaming);
    MouseDragOutcome {
        changed: *selection != before,
        lock_scroll,
    }
}

/// Finalize the selection state when the left button is released.
///
/// If the selection never became active (no head) or the head ended up equal to
/// the anchor, the selection is cleared so a click does not leave a persistent
/// highlight.
///
/// Returns whether the selection changed (useful to decide whether to request a
/// redraw).
pub(crate) fn on_mouse_up(selection: &mut TranscriptSelection) -> bool {
    let before = *selection;
    end(selection);
    *selection != before
}

/// Begin a potential selection by recording an anchor and clearing any head.
///
/// This ensures a plain click does not create an active selection/highlight.
/// The selection becomes active on the first drag that sets `head`.
fn begin(selection: &mut TranscriptSelection, point: TranscriptSelectionPoint) {
    *selection = TranscriptSelection {
        anchor: Some(point),
        head: None,
    };
}

/// Update selection state during a drag by setting `head` when anchored.
///
/// Returns whether the caller should lock the transcript scroll position while
/// streaming and following the bottom, so new output doesn't move the selection
/// under the cursor.
fn drag(
    selection: &mut TranscriptSelection,
    scroll: &TranscriptScroll,
    point: TranscriptSelectionPoint,
    streaming: bool,
) -> bool {
    let Some(anchor) = selection.anchor else {
        return false;
    };

    let should_lock_scroll =
        streaming && matches!(*scroll, TranscriptScroll::ToBottom) && point != anchor;

    selection.head = Some(point);

    should_lock_scroll
}

/// Finalize selection on mouse up.
///
/// Clears the selection if it never became active (no head) or if the head
/// ended up equal to the anchor, so a click doesn't leave a 1-cell highlight.
fn end(selection: &mut TranscriptSelection) {
    if selection.head.is_none() || selection.anchor == selection.head {
        *selection = TranscriptSelection::default();
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

    #[test]
    fn selection_only_highlights_on_drag() {
        let anchor = TranscriptSelectionPoint::new(0, 1);
        let head = TranscriptSelectionPoint::new(0, 3);

        let mut selection = TranscriptSelection::default();
        assert!(on_mouse_down(&mut selection, Some(anchor)));
        assert_eq!(
            selection,
            TranscriptSelection {
                anchor: Some(anchor),
                head: None,
            }
        );

        assert!(on_mouse_up(&mut selection));
        assert_eq!(selection, TranscriptSelection::default());

        assert!(on_mouse_down(&mut selection, Some(anchor)));
        let outcome = on_mouse_drag(
            &mut selection,
            &TranscriptScroll::ToBottom,
            Some(head),
            false,
        );
        assert!(outcome.changed);
        assert!(!outcome.lock_scroll);
        assert_eq!(
            selection,
            TranscriptSelection {
                anchor: Some(anchor),
                head: Some(head),
            }
        );
    }

    #[test]
    fn selection_clears_when_drag_ends_at_anchor() {
        let point = TranscriptSelectionPoint::new(0, 1);

        let mut selection = TranscriptSelection::default();
        assert!(on_mouse_down(&mut selection, Some(point)));
        let outcome = on_mouse_drag(
            &mut selection,
            &TranscriptScroll::ToBottom,
            Some(point),
            false,
        );
        assert!(outcome.changed);
        assert!(!outcome.lock_scroll);
        assert!(on_mouse_up(&mut selection));

        assert_eq!(selection, TranscriptSelection::default());
    }

    #[test]
    fn drag_requests_scroll_lock_when_streaming_at_bottom_and_point_moves() {
        let anchor = TranscriptSelectionPoint::new(0, 1);
        let head = TranscriptSelectionPoint::new(0, 2);

        let mut selection = TranscriptSelection::default();
        assert!(on_mouse_down(&mut selection, Some(anchor)));
        let outcome = on_mouse_drag(
            &mut selection,
            &TranscriptScroll::ToBottom,
            Some(head),
            true,
        );
        assert!(outcome.changed);
        assert!(outcome.lock_scroll);
    }

    #[test]
    fn selection_helpers_noop_without_points_or_anchor() {
        let mut selection = TranscriptSelection::default();
        assert!(!on_mouse_down(&mut selection, None));
        assert_eq!(selection, TranscriptSelection::default());

        let outcome = on_mouse_drag(&mut selection, &TranscriptScroll::ToBottom, None, false);
        assert_eq!(
            outcome,
            MouseDragOutcome {
                changed: false,
                lock_scroll: false,
            }
        );
        assert_eq!(selection, TranscriptSelection::default());

        let outcome = on_mouse_drag(
            &mut selection,
            &TranscriptScroll::ToBottom,
            Some(TranscriptSelectionPoint::new(0, 1)),
            false,
        );
        assert_eq!(
            outcome,
            MouseDragOutcome {
                changed: false,
                lock_scroll: false,
            }
        );
        assert_eq!(selection, TranscriptSelection::default());

        assert!(!on_mouse_up(&mut selection));
        assert_eq!(selection, TranscriptSelection::default());
    }

    #[test]
    fn mouse_down_resets_head() {
        let anchor = TranscriptSelectionPoint::new(0, 1);
        let head = TranscriptSelectionPoint::new(0, 2);
        let next_anchor = TranscriptSelectionPoint::new(1, 0);

        let mut selection = TranscriptSelection {
            anchor: Some(anchor),
            head: Some(head),
        };

        assert!(on_mouse_down(&mut selection, Some(next_anchor)));
        assert_eq!(
            selection,
            TranscriptSelection {
                anchor: Some(next_anchor),
                head: None,
            }
        );
    }

    #[test]
    fn dragging_does_not_request_scroll_lock_when_not_at_bottom() {
        let anchor = TranscriptSelectionPoint::new(0, 1);
        let head = TranscriptSelectionPoint::new(0, 2);

        let mut selection = TranscriptSelection::default();
        assert!(on_mouse_down(&mut selection, Some(anchor)));
        let outcome = on_mouse_drag(
            &mut selection,
            &TranscriptScroll::Scrolled {
                cell_index: 0,
                line_in_cell: 0,
            },
            Some(head),
            true,
        );
        assert!(outcome.changed);
        assert!(!outcome.lock_scroll);
    }
}
