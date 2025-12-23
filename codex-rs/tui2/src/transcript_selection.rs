//! Transcript selection primitives.
//!
//! The transcript (history) viewport is rendered as a flattened list of visual
//! lines after wrapping. Selection in the transcript needs to be stable across
//! scrolling and terminal resizes, so endpoints are expressed in
//! *content-relative* coordinates:
//!
//! - `line_index`: index into the flattened, wrapped transcript lines (visual
//!   lines).
//! - `column`: a zero-based offset within that visual line, measured from the
//!   first content column to the right of the gutter.
//!
//! These coordinates are intentionally independent of the current viewport: the
//! user can scroll after selecting, and the selection should continue to refer
//! to the same conversation content.
//!
//! Clipboard reconstruction is implemented in `transcript_copy` (including
//! off-screen lines), while keybinding detection and the on-screen copy
//! affordance live in `transcript_copy_ui`.
//!
//! ## Mouse selection semantics
//!
//! The transcript supports click-and-drag selection. To avoid leaving a
//! distracting 1-cell highlight on a simple click, the selection only becomes
//! active once a drag updates the head point.

use crate::tui::scrolling::TranscriptScroll;

/// Number of columns reserved for the transcript gutter (bullet/prefix space).
///
/// Transcript rendering prefixes each line with a short gutter (e.g. `• ` or
/// continuation padding). Selection coordinates intentionally exclude this
/// gutter so selection/copy operates on content columns instead of terminal
/// absolute columns.
pub(crate) const TRANSCRIPT_GUTTER_COLS: u16 = 2;

/// Content-relative selection within the inline transcript viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TranscriptSelection {
    /// The initial selection point (where the selection drag started).
    ///
    /// This remains fixed while dragging; the highlighted region is the span
    /// between `anchor` and `head`.
    pub(crate) anchor: Option<TranscriptSelectionPoint>,
    /// The current selection point (where the selection drag currently ends).
    ///
    /// This is `None` until the user drags, which prevents a simple click from
    /// creating a persistent selection highlight.
    pub(crate) head: Option<TranscriptSelectionPoint>,
}

/// A single endpoint of a transcript selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TranscriptSelectionPoint {
    /// Index into the flattened, wrapped transcript lines.
    pub(crate) line_index: usize,
    /// Zero-based content column (excluding the gutter).
    ///
    /// This is not a terminal absolute column: callers add the gutter offset
    /// when mapping it to a rendered buffer row.
    pub(crate) column: u16,
}

impl TranscriptSelectionPoint {
    /// Create a selection endpoint at a given wrapped line index and column.
    pub(crate) const fn new(line_index: usize, column: u16) -> Self {
        Self { line_index, column }
    }
}

impl From<(usize, u16)> for TranscriptSelectionPoint {
    fn from((line_index, column): (usize, u16)) -> Self {
        Self::new(line_index, column)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
