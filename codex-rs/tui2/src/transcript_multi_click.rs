//! Transcript-relative multi-click selection helpers.
//!
//! This module implements multi-click selection in terms of the **rendered
//! transcript model** (wrapped transcript lines + content columns), not
//! terminal buffer coordinates.
//!
//! Terminal `(row, col)` coordinates are ephemeral: scrolling, resizing, and
//! reflow (especially while streaming) change where a given piece of transcript
//! content appears on screen. Transcript-relative selection coordinates are
//! stable because they are anchored to the flattened, wrapped transcript line
//! model.
//!
//! Integration notes:
//! - Mouse event → `TranscriptSelectionPoint` mapping is handled by `app.rs`.
//! - This module:
//!   - groups nearby clicks into a multi-click sequence
//!   - expands the selection based on the current click count
//!   - rebuilds the wrapped transcript lines from `HistoryCell::display_lines(width)`
//!     so selection expansion matches on-screen wrapping.
//! - In TUI2 we start transcript selection on drag. A single click stores an
//!   anchor but is not an "active" selection (no head). Multi-click selection
//!   (double/triple/quad+) *does* create an active selection immediately.
//!
//! Complexity / cost model:
//! - single clicks are `O(1)` (just click tracking + caret placement)
//! - multi-click expansion rebuilds the current wrapped transcript view
//!   (`O(total rendered transcript text)`) so selection matches what is on screen
//!   *right now* (including streaming/reflow).
//!
//! Coordinates:
//! - `TranscriptSelectionPoint::line_index` is an index into the flattened,
//!   wrapped transcript lines ("visual lines").
//! - `TranscriptSelectionPoint::column` is a 0-based *content* column offset,
//!   measured from immediately after the transcript gutter
//!   (`TRANSCRIPT_GUTTER_COLS`).
//! - Selection endpoints are inclusive (they represent a closed interval of
//!   selected cells).
//!
//! Selection expansion is UI-oriented:
//! - "word" selection uses display width (`unicode_width`) and a lightweight
//!   character class heuristic.
//! - "paragraph" selection is based on contiguous non-empty wrapped lines.
//! - "cell" selection selects all wrapped lines that belong to a single history
//!   cell (the unit returned by `HistoryCell::display_lines`).

use crate::history_cell::HistoryCell;
use crate::transcript_selection::TRANSCRIPT_GUTTER_COLS;
use crate::transcript_selection::TranscriptSelection;
use crate::transcript_selection::TranscriptSelectionPoint;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use ratatui::text::Line;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

/// Stateful multi-click selection handler for the transcript viewport.
///
/// This holds the click history required to infer multi-click sequences across
/// mouse events. The actual selection expansion is computed from the current
/// transcript content so it stays aligned with on-screen wrapping.
#[derive(Debug, Default)]
pub(crate) struct TranscriptMultiClick {
    /// Tracks recent clicks so we can infer a multi-click sequence.
    ///
    /// This is intentionally kept separate from the selection itself: selection
    /// endpoints are owned by `TranscriptSelection`, while multi-click behavior
    /// is a transient input gesture state.
    tracker: ClickTracker,
}

impl TranscriptMultiClick {
    /// Handle a left-button mouse down within the transcript viewport.
    ///
    /// This is intended to be called from `App`'s mouse handler.
    ///
    /// Behavior:
    /// - Always updates the underlying selection anchor (delegates to
    ///   [`crate::transcript_selection::on_mouse_down`]) so dragging can extend
    ///   from this point.
    /// - Tracks the click as part of a potential multi-click sequence.
    /// - On multi-click (double/triple/quad+), replaces the selection with an
    ///   active expanded selection (word/line/paragraph).
    ///
    /// `width` must match the transcript viewport width used for rendering so
    /// wrapping (and therefore word/paragraph boundaries) align with what the
    /// user sees.
    ///
    /// Returns whether the selection changed (useful to decide whether to
    /// request a redraw).
    pub(crate) fn on_mouse_down(
        &mut self,
        selection: &mut TranscriptSelection,
        cells: &[Arc<dyn HistoryCell>],
        width: u16,
        point: Option<TranscriptSelectionPoint>,
    ) -> bool {
        self.on_mouse_down_at(selection, cells, width, point, Instant::now())
    }

    /// Notify the handler that the user is drag-selecting.
    ///
    /// Drag-selection should not be interpreted as a continuation of a
    /// multi-click sequence, so we reset click history once the cursor moves
    /// away from the anchor point.
    ///
    /// `point` is expected to be clamped to transcript content coordinates. If
    /// `point` is `None`, this is a no-op.
    pub(crate) fn on_mouse_drag(
        &mut self,
        selection: &TranscriptSelection,
        point: Option<TranscriptSelectionPoint>,
    ) {
        let (Some(anchor), Some(point)) = (selection.anchor, point) else {
            return;
        };

        // Some terminals emit `Drag` events for very small cursor motion while
        // the button is held down (e.g. trackpad “jitter” during a click).
        // Resetting the click sequence on *any* drag makes double/quad clicks
        // hard to trigger, so we only treat it as a drag gesture once the
        // cursor has meaningfully moved away from the anchor.
        let moved_to_other_wrapped_line = point.line_index != anchor.line_index;
        let moved_far_enough_horizontally =
            point.column.abs_diff(anchor.column) > ClickTracker::MAX_COLUMN_DISTANCE;
        if moved_to_other_wrapped_line || moved_far_enough_horizontally {
            self.tracker.reset();
        }
    }

    /// Testable implementation of [`Self::on_mouse_down`].
    ///
    /// Taking `now` as an input makes click grouping deterministic in tests.
    ///
    /// High-level flow (kept here so callers don’t have to mentally simulate the
    /// selection state machine):
    /// 1. Update the underlying selection state using
    ///    [`crate::transcript_selection::on_mouse_down`]. In TUI2 this records an
    ///    anchor and clears any head so a single click does not leave a visible
    ///    selection.
    /// 2. If the click is outside the transcript content (`point == None`),
    ///    reset the click tracker and return.
    /// 3. Register the click with the tracker to infer the click count.
    /// 4. For multi-click (`>= 2`), compute an expanded selection from the
    ///    *current* wrapped transcript view and overwrite the selection with an
    ///    active selection (`anchor` + `head` set).
    fn on_mouse_down_at(
        &mut self,
        selection: &mut TranscriptSelection,
        cells: &[Arc<dyn HistoryCell>],
        width: u16,
        point: Option<TranscriptSelectionPoint>,
        now: Instant,
    ) -> bool {
        let before = *selection;

        let selection_changed = crate::transcript_selection::on_mouse_down(selection, point);
        let Some(point) = point else {
            self.tracker.reset();
            return selection_changed;
        };

        let click_count = self.tracker.register_click(point, now);
        if click_count == 1 {
            return *selection != before;
        }

        *selection = selection_for_click(cells, width, point, click_count);
        *selection != before
    }
}

/// Tracks recent clicks so we can infer multi-click counts.
#[derive(Debug, Default)]
struct ClickTracker {
    /// The last click observed (used to group nearby clicks into a sequence).
    last_click: Option<Click>,
}

/// A single click event used for multi-click grouping.
#[derive(Debug, Clone, Copy)]
struct Click {
    /// Location of the click in transcript coordinates.
    point: TranscriptSelectionPoint,
    /// Click count for the current sequence.
    click_count: u8,
    /// Time the click occurred (used to bound multi-click grouping).
    at: Instant,
}

impl ClickTracker {
    /// Maximum time gap between clicks to be considered part of a sequence.
    const MAX_DELAY: Duration = Duration::from_millis(650);
    /// Maximum horizontal motion (in transcript *content* columns) to be
    /// considered "the same click target" for multi-click grouping.
    const MAX_COLUMN_DISTANCE: u16 = 4;

    /// Reset click history so the next click begins a new sequence.
    fn reset(&mut self) {
        self.last_click = None;
    }

    /// Record a click and return the inferred click count for this sequence.
    ///
    /// Clicks are grouped when:
    /// - they occur close in time (`MAX_DELAY`), and
    /// - they target the same transcript wrapped line, and
    /// - they occur at nearly the same content column (`MAX_COLUMN_DISTANCE`),
    ///   with increasing tolerance for later clicks in the sequence
    ///
    /// The returned count saturates at `u8::MAX` (we only care about the
    /// `>= 4` bucket).
    fn register_click(&mut self, point: TranscriptSelectionPoint, now: Instant) -> u8 {
        let mut click_count = 1u8;
        if let Some(prev) = self.last_click
            && now.duration_since(prev.at) <= Self::MAX_DELAY
            && prev.point.line_index == point.line_index
            && prev.point.column.abs_diff(point.column) <= max_column_distance(prev.click_count)
        {
            click_count = prev.click_count.saturating_add(1);
        }

        self.last_click = Some(Click {
            point,
            click_count,
            at: now,
        });

        click_count
    }
}

/// Column-distance tolerance for continuing an existing click sequence.
///
/// We intentionally loosen grouping after the selection has expanded: once the
/// user is on the “whole line” or “paragraph” step, requiring a near-identical
/// column makes quad-clicks hard to trigger because the user can naturally
/// click elsewhere on the already-highlighted line.
fn max_column_distance(prev_click_count: u8) -> u16 {
    match prev_click_count {
        0 | 1 => ClickTracker::MAX_COLUMN_DISTANCE,
        2 => ClickTracker::MAX_COLUMN_DISTANCE.saturating_mul(2),
        _ => u16::MAX,
    }
}

/// Expand a click (plus inferred `click_count`) into a transcript selection.
///
/// This is the core of multi-click behavior. For expanded selections it
/// rebuilds the current wrapped transcript view from history cells so selection
/// boundaries line up with the rendered transcript model (not raw source
/// strings, and not terminal buffer coordinates).
///
/// `TranscriptSelectionPoint::column` is interpreted in content coordinates:
/// column 0 is the first column immediately after the transcript gutter
/// (`TRANSCRIPT_GUTTER_COLS`). The returned selection columns are clamped to
/// the content width for the given `width`.
///
/// Gesture mapping:
/// - double click selects a “word-ish” run on the clicked wrapped line
/// - triple click selects the entire wrapped line
/// - quad+ click selects the containing paragraph (contiguous non-empty wrapped
///   lines, with empty/spacer lines treated as paragraph breaks)
/// - quint+ click selects the entire history cell
///
/// Returned selections are always “active” (both `anchor` and `head` set). This
/// intentionally differs from normal single-click behavior in TUI2 (which only
/// stores an anchor until a drag makes the selection active).
///
/// Defensiveness:
/// - if the transcript is empty, or wrapping yields no lines, this falls back
///   to a caret-like selection at `point` so multi-click never produces “no
///   selection”
/// - if `point` refers past the end of the wrapped line list, it is clamped to
///   the last wrapped line so behavior stays stable during scroll/resize/reflow
fn selection_for_click(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    point: TranscriptSelectionPoint,
    click_count: u8,
) -> TranscriptSelection {
    if click_count == 1 {
        return TranscriptSelection {
            anchor: Some(point),
            head: Some(point),
        };
    }

    // `width` is the total viewport width, including the gutter. Selection
    // columns are content-relative, so compute the maximum selectable *content*
    // column.
    let max_content_col = width
        .saturating_sub(1)
        .saturating_sub(TRANSCRIPT_GUTTER_COLS);

    // Rebuild the same logical line stream the transcript renders from. This
    // keeps expansion boundaries aligned with current streaming output and the
    // current wrap width.
    let (lines, line_cell_index) = build_transcript_lines_with_cell_index(cells, width);
    if lines.is_empty() {
        return TranscriptSelection {
            anchor: Some(point),
            head: Some(point),
        };
    }

    // Expand based on the wrapped *visual* lines so triple/quad/quint-click
    // selection respects the current wrap width.
    let (wrapped, wrapped_cell_index) = word_wrap_lines_with_cell_index(
        &lines,
        &line_cell_index,
        RtOptions::new(width.max(1) as usize),
    );
    if wrapped.is_empty() {
        return TranscriptSelection {
            anchor: Some(point),
            head: Some(point),
        };
    }

    // Clamp both the target line and column into the current wrapped view. This
    // matters during live streaming, where the transcript can grow between the
    // time the UI clamps the click and the time we compute expansion.
    let line_index = point.line_index.min(wrapped.len().saturating_sub(1));
    let point = TranscriptSelectionPoint::new(line_index, point.column.min(max_content_col));

    if click_count == 2 {
        let Some((start, end)) =
            word_bounds_in_wrapped_line(&wrapped[line_index], TRANSCRIPT_GUTTER_COLS, point.column)
        else {
            return TranscriptSelection {
                anchor: Some(point),
                head: Some(point),
            };
        };
        return TranscriptSelection {
            anchor: Some(TranscriptSelectionPoint::new(
                line_index,
                start.min(max_content_col),
            )),
            head: Some(TranscriptSelectionPoint::new(
                line_index,
                end.min(max_content_col),
            )),
        };
    }

    if click_count == 3 {
        return TranscriptSelection {
            anchor: Some(TranscriptSelectionPoint::new(line_index, 0)),
            head: Some(TranscriptSelectionPoint::new(line_index, max_content_col)),
        };
    }

    if click_count == 4 {
        let (start_line, end_line) =
            paragraph_bounds_in_wrapped_lines(&wrapped, TRANSCRIPT_GUTTER_COLS, line_index)
                .unwrap_or((line_index, line_index));
        return TranscriptSelection {
            anchor: Some(TranscriptSelectionPoint::new(start_line, 0)),
            head: Some(TranscriptSelectionPoint::new(end_line, max_content_col)),
        };
    }

    let Some((start_line, end_line)) =
        cell_bounds_in_wrapped_lines(&wrapped_cell_index, line_index)
    else {
        return TranscriptSelection {
            anchor: Some(point),
            head: Some(point),
        };
    };
    TranscriptSelection {
        anchor: Some(TranscriptSelectionPoint::new(start_line, 0)),
        head: Some(TranscriptSelectionPoint::new(end_line, max_content_col)),
    }
}

/// Flatten transcript history cells into the same line stream used by the UI.
///
/// This mirrors `App::build_transcript_lines` semantics: insert a blank spacer
/// line between non-continuation cells so word/paragraph boundaries match what
/// the user sees.
#[cfg(test)]
fn build_transcript_lines(cells: &[Arc<dyn HistoryCell>], width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut has_emitted_lines = false;

    for cell in cells {
        let cell_lines = cell.display_lines(width);
        if cell_lines.is_empty() {
            continue;
        }

        if !cell.is_stream_continuation() {
            if has_emitted_lines {
                // `App` inserts a spacer between distinct (non-continuation)
                // history cells; preserve that here so paragraph detection
                // matches what users see.
                lines.push(Line::from(""));
            } else {
                has_emitted_lines = true;
            }
        }

        lines.extend(cell_lines);
    }

    lines
}

/// Like [`build_transcript_lines`], but also returns a per-line mapping to the
/// originating history cell index.
///
/// This mapping lets us implement "select the whole history cell" in terms of
/// wrapped visual line indices.
fn build_transcript_lines_with_cell_index(
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
) -> (Vec<Line<'static>>, Vec<Option<usize>>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line_cell_index: Vec<Option<usize>> = Vec::new();
    let mut has_emitted_lines = false;

    for (cell_index, cell) in cells.iter().enumerate() {
        let cell_lines = cell.display_lines(width);
        if cell_lines.is_empty() {
            continue;
        }

        if !cell.is_stream_continuation() {
            if has_emitted_lines {
                lines.push(Line::from(""));
                line_cell_index.push(None);
            } else {
                has_emitted_lines = true;
            }
        }

        line_cell_index.extend(std::iter::repeat_n(Some(cell_index), cell_lines.len()));
        lines.extend(cell_lines);
    }

    debug_assert_eq!(lines.len(), line_cell_index.len());
    (lines, line_cell_index)
}

/// Wrap lines and carry forward a per-line mapping to history cell index.
///
/// This mirrors [`word_wrap_lines_borrowed`] behavior so selection expansion
/// uses the same wrapped line model as rendering.
fn word_wrap_lines_with_cell_index<'a, O>(
    lines: &'a [Line<'a>],
    line_cell_index: &[Option<usize>],
    width_or_options: O,
) -> (Vec<Line<'a>>, Vec<Option<usize>>)
where
    O: Into<RtOptions<'a>>,
{
    debug_assert_eq!(lines.len(), line_cell_index.len());

    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'a>> = Vec::new();
    let mut out_cell_index: Vec<Option<usize>> = Vec::new();

    let mut first = true;
    for (line, cell_index) in lines.iter().zip(line_cell_index.iter().copied()) {
        let opts = if first {
            base_opts.clone()
        } else {
            base_opts
                .clone()
                .initial_indent(base_opts.subsequent_indent.clone())
        };

        let wrapped = word_wrap_line(line, opts);
        out_cell_index.extend(std::iter::repeat_n(cell_index, wrapped.len()));
        out.extend(wrapped);
        first = false;
    }

    debug_assert_eq!(out.len(), out_cell_index.len());
    (out, out_cell_index)
}

/// Expand to the contiguous range of wrapped lines that belong to a single
/// history cell.
///
/// `line_index` is in wrapped line coordinates. If the line at `line_index` is
/// a spacer (no cell index), we select the nearest preceding cell, falling back
/// to the next cell below.
fn cell_bounds_in_wrapped_lines(
    wrapped_cell_index: &[Option<usize>],
    line_index: usize,
) -> Option<(usize, usize)> {
    let total = wrapped_cell_index.len();
    if total == 0 {
        return None;
    }

    let mut target = line_index.min(total.saturating_sub(1));
    let mut cell_index = wrapped_cell_index[target];
    if cell_index.is_none() {
        if let Some(found) = (0..target)
            .rev()
            .find(|idx| wrapped_cell_index[*idx].is_some())
        {
            target = found;
            cell_index = wrapped_cell_index[found];
        } else if let Some(found) =
            (target + 1..total).find(|idx| wrapped_cell_index[*idx].is_some())
        {
            target = found;
            cell_index = wrapped_cell_index[found];
        }
    }
    let cell_index = cell_index?;

    let mut start = target;
    while start > 0 && wrapped_cell_index[start - 1] == Some(cell_index) {
        start = start.saturating_sub(1);
    }

    let mut end = target;
    while end + 1 < total && wrapped_cell_index[end + 1] == Some(cell_index) {
        end = end.saturating_add(1);
    }

    Some((start, end))
}

/// Coarse character classes used for "word-ish" selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordCharClass {
    /// Any whitespace (select as a contiguous run).
    Whitespace,
    /// Alphanumeric plus token punctuation (paths/idents/URLs).
    Token,
    /// Everything else.
    Other,
}

/// Classify characters for UI-oriented "word-ish" selection.
///
/// This intentionally does not attempt full Unicode word boundary semantics.
/// It is tuned for terminal transcript interactions, where "word" often means
/// identifiers, paths, URLs, and punctuation-adjacent tokens.
fn word_char_class(ch: char) -> WordCharClass {
    if ch.is_whitespace() {
        return WordCharClass::Whitespace;
    }

    let is_token = ch.is_alphanumeric()
        || matches!(
            ch,
            '_' | '-'
                | '.'
                | '/'
                | '\\'
                | ':'
                | '@'
                | '#'
                | '$'
                | '%'
                | '+'
                | '='
                | '?'
                | '&'
                | '~'
                | '*'
        );
    if is_token {
        WordCharClass::Token
    } else {
        WordCharClass::Other
    }
}

/// Concatenate a styled `Line` into its plain text representation.
///
/// Multi-click selection operates on the rendered text content (what the user
/// sees), independent of styling.
fn flatten_line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Find the UTF-8 byte index that corresponds to `prefix_cols` display columns.
///
/// This is used to exclude the transcript gutter/prefix when interpreting
/// clicks and paragraph breaks. Column math uses display width, not byte
/// offsets, to match terminal layout.
fn byte_index_after_prefix_cols(text: &str, prefix_cols: u16) -> usize {
    let mut col = 0u16;
    for (idx, ch) in text.char_indices() {
        if col >= prefix_cols {
            return idx;
        }
        col = col.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0) as u16);
    }
    text.len()
}

/// Compute the (inclusive) content column bounds of the "word" under a click.
///
/// This is defined in terms of the *rendered* line:
/// - `line` is a visual wrapped transcript line (including the gutter/prefix).
/// - `prefix_cols` is the number of display columns to ignore on the left
///   (the transcript gutter).
/// - `click_col` is a 0-based content column, measured from the first column
///   after the gutter.
///
/// The returned `(start, end)` is an inclusive selection range in content
/// columns (`0..=max_content_col`), suitable for populating
/// [`TranscriptSelectionPoint::column`].
fn word_bounds_in_wrapped_line(
    line: &Line<'_>,
    prefix_cols: u16,
    click_col: u16,
) -> Option<(u16, u16)> {
    // We compute word bounds by flattening to plain text and mapping each
    // displayed glyph to a column range (by display width). This mirrors what
    // the user sees, even if the underlying spans have multiple styles.
    //
    // Notes / limitations:
    // - This operates at the `char` level, not grapheme clusters. For most
    //   transcript content (ASCII-ish tokens/paths/URLs) that’s sufficient.
    // - Zero-width chars are skipped; they don’t occupy a terminal cell.
    let full = flatten_line_text(line);
    let prefix_byte = byte_index_after_prefix_cols(&full, prefix_cols);
    let content = &full[prefix_byte..];

    let mut cells: Vec<(char, u16, u16)> = Vec::new();
    let mut col = 0u16;
    for ch in content.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 {
            continue;
        }
        let start = col;
        let end = col.saturating_add(w);
        cells.push((ch, start, end));
        col = end;
    }

    let total_width = col;
    if cells.is_empty() || total_width == 0 {
        return None;
    }

    let click_col = click_col.min(total_width.saturating_sub(1));
    let mut idx = cells
        .iter()
        .position(|(_, start, end)| click_col >= *start && click_col < *end)
        .unwrap_or(0);
    if idx >= cells.len() {
        idx = cells.len().saturating_sub(1);
    }

    let class = word_char_class(cells[idx].0);

    let mut start_idx = idx;
    while start_idx > 0 && word_char_class(cells[start_idx - 1].0) == class {
        start_idx = start_idx.saturating_sub(1);
    }

    let mut end_idx = idx;
    while end_idx + 1 < cells.len() && word_char_class(cells[end_idx + 1].0) == class {
        end_idx = end_idx.saturating_add(1);
    }

    let start_col = cells[start_idx].1;
    let end_col = cells[end_idx].2.saturating_sub(1);
    Some((start_col, end_col))
}

/// Compute the (inclusive) wrapped line index bounds of the paragraph
/// surrounding `line_index`.
///
/// Paragraphs are defined on *wrapped visual lines* (not underlying history
/// cells): a paragraph is any contiguous run of non-empty wrapped lines, and
/// empty lines (after trimming the transcript gutter/prefix) break paragraphs.
///
/// When `line_index` points at a break line, this selects the nearest preceding
/// non-break line so a quad-click on the spacer line between history cells
/// selects the paragraph above (matching common terminal UX expectations).
fn paragraph_bounds_in_wrapped_lines(
    lines: &[Line<'_>],
    prefix_cols: u16,
    line_index: usize,
) -> Option<(usize, usize)> {
    if lines.is_empty() {
        return None;
    }

    // Paragraph breaks are determined after skipping the transcript gutter so a
    // line that only contains the gutter prefix still counts as “empty”.
    let is_break = |idx: usize| -> bool {
        let full = flatten_line_text(&lines[idx]);
        let prefix_byte = byte_index_after_prefix_cols(&full, prefix_cols);
        full[prefix_byte..].trim().is_empty()
    };

    let mut target = line_index.min(lines.len().saturating_sub(1));
    if is_break(target) {
        // Prefer the paragraph above for spacer lines inserted between history
        // cells. If there is no paragraph above, fall back to the next
        // paragraph below.
        target = (0..target)
            .rev()
            .find(|idx| !is_break(*idx))
            .or_else(|| (target + 1..lines.len()).find(|idx| !is_break(*idx)))?;
    }

    let mut start = target;
    while start > 0 && !is_break(start - 1) {
        start = start.saturating_sub(1);
    }

    let mut end = target;
    while end + 1 < lines.len() && !is_break(end + 1) {
        end = end.saturating_add(1);
    }

    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::text::Line;

    #[derive(Debug)]
    struct StaticCell {
        lines: Vec<Line<'static>>,
        is_stream_continuation: bool,
    }

    impl StaticCell {
        fn new(lines: Vec<Line<'static>>) -> Self {
            Self {
                lines,
                is_stream_continuation: false,
            }
        }

        fn continuation(lines: Vec<Line<'static>>) -> Self {
            Self {
                lines,
                is_stream_continuation: true,
            }
        }
    }

    impl HistoryCell for StaticCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn is_stream_continuation(&self) -> bool {
            self.is_stream_continuation
        }
    }

    #[test]
    fn word_bounds_respects_prefix_and_word_classes() {
        let line = Line::from("› hello   world");
        let prefix_cols = 2;

        assert_eq!(
            word_bounds_in_wrapped_line(&line, prefix_cols, 1),
            Some((0, 4))
        );
        assert_eq!(
            word_bounds_in_wrapped_line(&line, prefix_cols, 6),
            Some((5, 7))
        );
        assert_eq!(
            word_bounds_in_wrapped_line(&line, prefix_cols, 9),
            Some((8, 12))
        );
    }

    #[test]
    fn paragraph_bounds_selects_contiguous_non_empty_lines() {
        let lines = vec![
            Line::from("› first"),
            Line::from("  second"),
            Line::from(""),
            Line::from("› third"),
        ];
        let prefix_cols = 2;

        assert_eq!(
            paragraph_bounds_in_wrapped_lines(&lines, prefix_cols, 1),
            Some((0, 1))
        );
        assert_eq!(
            paragraph_bounds_in_wrapped_lines(&lines, prefix_cols, 2),
            Some((0, 1))
        );
        assert_eq!(
            paragraph_bounds_in_wrapped_lines(&lines, prefix_cols, 3),
            Some((3, 3))
        );
    }

    #[test]
    fn click_sequence_expands_selection_word_then_line_then_paragraph() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(StaticCell::new(vec![
            Line::from("› first"),
            Line::from("  second"),
        ]))];
        let width = 20;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(1, 1);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        assert_eq!(selection.anchor, Some(point));
        assert_eq!(selection.head, None);

        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(10),
        );
        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.line_index, a.column, h.column)),
            Some((1, 0, 5))
        );

        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(20),
        );
        let max_content_col = width
            .saturating_sub(1)
            .saturating_sub(TRANSCRIPT_GUTTER_COLS);
        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.line_index, a.column, h.column)),
            Some((1, 0, max_content_col))
        );

        // The final click can land elsewhere on the highlighted line; we still
        // want to treat it as continuing the multi-click sequence.
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(point.line_index, 10)),
            t0 + Duration::from_millis(30),
        );
        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.line_index, h.line_index)),
            Some((0, 1))
        );
    }

    #[test]
    fn double_click_on_whitespace_selects_whitespace_run() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(StaticCell::new(vec![Line::from(
            "› hello   world",
        )]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(0, 6);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(5),
        );

        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.column, h.column)),
            Some((5, 7))
        );
    }

    #[test]
    fn click_sequence_resets_when_click_moves_too_far_horizontally() {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(StaticCell::new(vec![Line::from("› hello world")]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(0, 0)),
            t0,
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(0, 10)),
            t0 + Duration::from_millis(10),
        );

        assert_eq!(selection.anchor, Some(TranscriptSelectionPoint::new(0, 10)));
        assert_eq!(selection.head, None);
    }

    #[test]
    fn click_sequence_resets_when_click_is_too_slow() {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(StaticCell::new(vec![Line::from("› hello world")]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(0, 1);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + ClickTracker::MAX_DELAY + Duration::from_millis(1),
        );

        assert_eq!(selection.anchor, Some(point));
        assert_eq!(selection.head, None);
    }

    #[test]
    fn click_sequence_resets_when_click_changes_line() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(StaticCell::new(vec![
            Line::from("› first"),
            Line::from("  second"),
        ]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(0, 1)),
            t0,
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(1, 1)),
            t0 + Duration::from_millis(10),
        );

        assert_eq!(selection.anchor, Some(TranscriptSelectionPoint::new(1, 1)));
        assert_eq!(selection.head, None);
    }

    #[test]
    fn drag_resets_multi_click_sequence() {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(StaticCell::new(vec![Line::from("› hello world")]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(0, 1);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(10),
        );
        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.column, h.column)),
            Some((0, 4))
        );

        multi.on_mouse_drag(&selection, Some(TranscriptSelectionPoint::new(0, 10)));
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(20),
        );
        assert_eq!(selection.anchor, Some(point));
        assert_eq!(selection.head, None);
    }

    #[test]
    fn small_drag_jitter_does_not_reset_multi_click_sequence() {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(StaticCell::new(vec![Line::from("› hello world")]))];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(0, 1);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_drag(&selection, Some(TranscriptSelectionPoint::new(0, 2)));

        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(10),
        );
        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.column, h.column)),
            Some((0, 4))
        );
    }

    #[test]
    fn paragraph_selects_nearest_non_empty_when_clicking_break_line() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(StaticCell::new(vec![Line::from("› first")])),
            Arc::new(StaticCell::new(vec![Line::from("› second")])),
        ];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let mut selection = TranscriptSelection::default();

        // Index 1 is the spacer line inserted between the two non-continuation cells.
        let point = TranscriptSelectionPoint::new(1, 0);
        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(10),
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(20),
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(30),
        );

        assert_eq!(
            selection
                .anchor
                .zip(selection.head)
                .map(|(a, h)| (a.line_index, h.line_index)),
            Some((0, 0))
        );
    }

    #[test]
    fn build_transcript_lines_inserts_spacer_between_non_continuation_cells() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(StaticCell::new(vec![Line::from("› first")])),
            Arc::new(StaticCell::continuation(vec![Line::from("  cont")])),
            Arc::new(StaticCell::new(vec![Line::from("› second")])),
        ];
        let width = 40;

        let lines = build_transcript_lines(&cells, width);
        let text: Vec<String> = lines.iter().map(flatten_line_text).collect();
        assert_eq!(text, vec!["› first", "  cont", "", "› second"]);
    }

    #[test]
    fn quint_click_selects_entire_history_cell() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(StaticCell::new(vec![
                Line::from("› first"),
                Line::from(""),
                Line::from("  second"),
            ])),
            Arc::new(StaticCell::new(vec![Line::from("› other")])),
        ];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let point = TranscriptSelectionPoint::new(2, 1);
        let mut selection = TranscriptSelection::default();

        multi.on_mouse_down_at(&mut selection, &cells, width, Some(point), t0);
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(10),
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(20),
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(point),
            t0 + Duration::from_millis(30),
        );
        multi.on_mouse_down_at(
            &mut selection,
            &cells,
            width,
            Some(TranscriptSelectionPoint::new(2, 10)),
            t0 + Duration::from_millis(40),
        );

        let max_content_col = width
            .saturating_sub(1)
            .saturating_sub(TRANSCRIPT_GUTTER_COLS);
        assert_eq!(
            selection.anchor.zip(selection.head).map(|(a, h)| (
                a.line_index,
                a.column,
                h.line_index,
                h.column
            )),
            Some((0, 0, 2, max_content_col))
        );
    }

    #[test]
    fn quint_click_on_spacer_selects_cell_above() {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![
            Arc::new(StaticCell::new(vec![Line::from("› first")])),
            Arc::new(StaticCell::new(vec![Line::from("› second")])),
        ];
        let width = 40;

        let mut multi = TranscriptMultiClick::default();
        let t0 = Instant::now();
        let mut selection = TranscriptSelection::default();

        // Index 1 is the spacer line inserted between the two non-continuation cells.
        let point = TranscriptSelectionPoint::new(1, 0);
        for (idx, dt) in [0u64, 10, 20, 30, 40].into_iter().enumerate() {
            multi.on_mouse_down_at(
                &mut selection,
                &cells,
                width,
                Some(TranscriptSelectionPoint::new(
                    point.line_index,
                    if idx < 3 { 0 } else { (idx as u16) * 5 },
                )),
                t0 + Duration::from_millis(dt),
            );
        }

        let max_content_col = width
            .saturating_sub(1)
            .saturating_sub(TRANSCRIPT_GUTTER_COLS);
        assert_eq!(
            selection.anchor.zip(selection.head).map(|(a, h)| (
                a.line_index,
                a.column,
                h.line_index,
                h.column
            )),
            Some((0, 0, 0, max_content_col))
        );
    }
}
