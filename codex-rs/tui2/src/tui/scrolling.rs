//! Inline transcript scrolling primitives.
//!
//! The TUI renders the transcript as a list of logical *cells* (user prompts, agent responses,
//! banners, etc.). Each frame flattens those cells into a sequence of visual lines (after wrapping)
//! plus a parallel `line_meta` vector that maps each visual line back to its origin
//! (`TranscriptLineMeta`) (see `App::build_transcript_lines` and the design notes in
//! `codex-rs/tui2/docs/tui_viewport_and_history.md`).
//!
//! This module defines the scroll state for the inline transcript viewport and helpers to:
//! - Resolve that state into a concrete top-row offset for the current frame.
//! - Apply a scroll delta (mouse wheel / PgUp / PgDn) in terms of *visual lines*.
//! - Convert a concrete top-row offset back into a stable anchor.
//!
//! Why anchors instead of a raw "top row" index?
//! - When the transcript grows, a raw index drifts relative to the user's chosen content.
//! - By anchoring to a particular `(cell_index, line_in_cell)`, we can re-find the same content in
//!   the newly flattened line list on the next frame.
//!
//! Spacer rows between non-continuation cells are represented as `TranscriptLineMeta::Spacer`.
//! They are not valid anchors; `anchor_for` will pick the nearest non-spacer line when needed.

/// Per-flattened-line metadata for the transcript view.
///
/// Each rendered line in the flattened transcript has a corresponding `TranscriptLineMeta` entry
/// describing where that visual line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptLineMeta {
    /// A visual line that belongs to a transcript cell.
    CellLine {
        cell_index: usize,
        line_in_cell: usize,
    },
    /// A synthetic spacer row inserted between non-continuation cells.
    Spacer,
}

impl TranscriptLineMeta {
    pub(crate) fn cell_line(&self) -> Option<(usize, usize)> {
        match *self {
            Self::CellLine {
                cell_index,
                line_in_cell,
            } => Some((cell_index, line_in_cell)),
            Self::Spacer => None,
        }
    }

    pub(crate) fn cell_index(&self) -> Option<usize> {
        match *self {
            Self::CellLine { cell_index, .. } => Some(cell_index),
            Self::Spacer => None,
        }
    }
}

/// Scroll state for the inline transcript viewport.
///
/// This tracks whether the transcript is pinned to the latest line or anchored
/// at a specific cell/line pair so later viewport changes can implement
/// scrollback without losing the notion of "bottom".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TranscriptScroll {
    #[default]
    /// Follow the most recent line in the transcript.
    ToBottom,
    /// Anchor the viewport to a specific transcript cell and line.
    ///
    /// `cell_index` indexes into the logical transcript cell list. `line_in_cell` is the 0-based
    /// visual line index within that cell as produced by the current wrapping/layout.
    Scrolled {
        cell_index: usize,
        line_in_cell: usize,
    },
}

impl TranscriptScroll {
    /// Resolve the top row for the current scroll state.
    ///
    /// `line_meta` is a line-parallel mapping of flattened transcript lines.
    ///
    /// `max_start` is the maximum valid top-row offset for the current viewport height (i.e. the
    /// last scroll position that still yields a full viewport of content).
    ///
    /// Returns the (possibly updated) scroll state plus the resolved top-row offset. If the current
    /// anchor can no longer be found in `line_meta` (for example because the transcript was
    /// truncated), this falls back to `ToBottom` so the UI stays usable.
    pub(crate) fn resolve_top(
        self,
        line_meta: &[TranscriptLineMeta],
        max_start: usize,
    ) -> (Self, usize) {
        match self {
            Self::ToBottom => (Self::ToBottom, max_start),
            Self::Scrolled {
                cell_index,
                line_in_cell,
            } => {
                let anchor = anchor_index(line_meta, cell_index, line_in_cell);
                match anchor {
                    Some(idx) => (self, idx.min(max_start)),
                    None => (Self::ToBottom, max_start),
                }
            }
        }
    }

    /// Apply a scroll delta and return the updated scroll state.
    ///
    /// `delta_lines` is in *visual lines* (after wrapping): negative deltas scroll upward into
    /// scrollback, positive deltas scroll downward toward the latest content.
    ///
    /// See `resolve_top` for `line_meta` semantics. `visible_lines` is the viewport height in rows.
    /// If all flattened lines fit in the viewport, this always returns `ToBottom`.
    pub(crate) fn scrolled_by(
        self,
        delta_lines: i32,
        line_meta: &[TranscriptLineMeta],
        visible_lines: usize,
    ) -> Self {
        if delta_lines == 0 {
            return self;
        }

        let total_lines = line_meta.len();
        if total_lines <= visible_lines {
            return Self::ToBottom;
        }

        let max_start = total_lines.saturating_sub(visible_lines);
        let current_top = match self {
            Self::ToBottom => max_start,
            Self::Scrolled {
                cell_index,
                line_in_cell,
            } => anchor_index(line_meta, cell_index, line_in_cell)
                .unwrap_or(max_start)
                .min(max_start),
        };

        let new_top = if delta_lines < 0 {
            current_top.saturating_sub(delta_lines.unsigned_abs() as usize)
        } else {
            current_top
                .saturating_add(delta_lines as usize)
                .min(max_start)
        };

        if new_top == max_start {
            return Self::ToBottom;
        }

        Self::anchor_for(line_meta, new_top).unwrap_or(Self::ToBottom)
    }

    /// Anchor to the first available line at or near the given start offset.
    ///
    /// This is the inverse of "resolving a scroll state to a top-row offset":
    /// given a concrete flattened line index, pick a stable `(cell_index, line_in_cell)` anchor.
    ///
    /// See `resolve_top` for `line_meta` semantics. This prefers the nearest line at or after `start`
    /// (skipping spacer rows), falling back to the nearest line before it when needed.
    pub(crate) fn anchor_for(line_meta: &[TranscriptLineMeta], start: usize) -> Option<Self> {
        let anchor =
            anchor_at_or_after(line_meta, start).or_else(|| anchor_at_or_before(line_meta, start));
        anchor.map(|(cell_index, line_in_cell)| Self::Scrolled {
            cell_index,
            line_in_cell,
        })
    }
}

/// Locate the flattened line index for a specific transcript cell and line.
///
/// This scans `meta` for the exact `(cell_index, line_in_cell)` anchor. It returns `None` when the
/// anchor is not present in the current frame's flattened line list (for example if a cell was
/// removed or its displayed line count changed).
fn anchor_index(
    line_meta: &[TranscriptLineMeta],
    cell_index: usize,
    line_in_cell: usize,
) -> Option<usize> {
    line_meta
        .iter()
        .enumerate()
        .find_map(|(idx, entry)| match *entry {
            TranscriptLineMeta::CellLine {
                cell_index: ci,
                line_in_cell: li,
            } if ci == cell_index && li == line_in_cell => Some(idx),
            _ => None,
        })
}

/// Find the first transcript line at or after the given flattened index.
fn anchor_at_or_after(line_meta: &[TranscriptLineMeta], start: usize) -> Option<(usize, usize)> {
    if line_meta.is_empty() {
        return None;
    }
    let start = start.min(line_meta.len().saturating_sub(1));
    line_meta
        .iter()
        .skip(start)
        .find_map(TranscriptLineMeta::cell_line)
}

/// Find the nearest transcript line at or before the given flattened index.
fn anchor_at_or_before(line_meta: &[TranscriptLineMeta], start: usize) -> Option<(usize, usize)> {
    if line_meta.is_empty() {
        return None;
    }
    let start = start.min(line_meta.len().saturating_sub(1));
    line_meta[..=start]
        .iter()
        .rev()
        .find_map(TranscriptLineMeta::cell_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn meta(entries: &[TranscriptLineMeta]) -> Vec<TranscriptLineMeta> {
        entries.to_vec()
    }

    fn cell_line(cell_index: usize, line_in_cell: usize) -> TranscriptLineMeta {
        TranscriptLineMeta::CellLine {
            cell_index,
            line_in_cell,
        }
    }

    #[test]
    fn resolve_top_to_bottom_clamps_to_max_start() {
        let meta = meta(&[
            cell_line(0, 0),
            cell_line(0, 1),
            TranscriptLineMeta::Spacer,
            cell_line(1, 0),
        ]);

        let (state, top) = TranscriptScroll::ToBottom.resolve_top(&meta, 3);

        assert_eq!(state, TranscriptScroll::ToBottom);
        assert_eq!(top, 3);
    }

    #[test]
    fn resolve_top_scrolled_keeps_anchor_when_present() {
        let meta = meta(&[
            cell_line(0, 0),
            TranscriptLineMeta::Spacer,
            cell_line(1, 0),
            cell_line(1, 1),
        ]);
        let scroll = TranscriptScroll::Scrolled {
            cell_index: 1,
            line_in_cell: 0,
        };

        let (state, top) = scroll.resolve_top(&meta, 2);

        assert_eq!(state, scroll);
        assert_eq!(top, 2);
    }

    #[test]
    fn resolve_top_scrolled_falls_back_when_anchor_missing() {
        let meta = meta(&[cell_line(0, 0), TranscriptLineMeta::Spacer, cell_line(1, 0)]);
        let scroll = TranscriptScroll::Scrolled {
            cell_index: 2,
            line_in_cell: 0,
        };

        let (state, top) = scroll.resolve_top(&meta, 1);

        assert_eq!(state, TranscriptScroll::ToBottom);
        assert_eq!(top, 1);
    }

    #[test]
    fn scrolled_by_moves_upward_and_anchors() {
        let meta = meta(&[
            cell_line(0, 0),
            cell_line(0, 1),
            cell_line(1, 0),
            TranscriptLineMeta::Spacer,
            cell_line(2, 0),
            cell_line(2, 1),
        ]);

        let state = TranscriptScroll::ToBottom.scrolled_by(-1, &meta, 3);

        assert_eq!(
            state,
            TranscriptScroll::Scrolled {
                cell_index: 1,
                line_in_cell: 0
            }
        );
    }

    #[test]
    fn scrolled_by_returns_to_bottom_when_scrolling_down() {
        let meta = meta(&[
            cell_line(0, 0),
            cell_line(0, 1),
            cell_line(1, 0),
            cell_line(2, 0),
        ]);
        let scroll = TranscriptScroll::Scrolled {
            cell_index: 0,
            line_in_cell: 0,
        };

        let state = scroll.scrolled_by(5, &meta, 2);

        assert_eq!(state, TranscriptScroll::ToBottom);
    }

    #[test]
    fn scrolled_by_to_bottom_when_all_lines_fit() {
        let meta = meta(&[cell_line(0, 0), cell_line(0, 1)]);

        let state = TranscriptScroll::Scrolled {
            cell_index: 0,
            line_in_cell: 0,
        }
        .scrolled_by(-1, &meta, 5);

        assert_eq!(state, TranscriptScroll::ToBottom);
    }

    #[test]
    fn anchor_for_prefers_after_then_before() {
        let meta = meta(&[
            TranscriptLineMeta::Spacer,
            cell_line(0, 0),
            TranscriptLineMeta::Spacer,
            cell_line(1, 0),
        ]);

        assert_eq!(
            TranscriptScroll::anchor_for(&meta, 0),
            Some(TranscriptScroll::Scrolled {
                cell_index: 0,
                line_in_cell: 0
            })
        );
        assert_eq!(
            TranscriptScroll::anchor_for(&meta, 2),
            Some(TranscriptScroll::Scrolled {
                cell_index: 1,
                line_in_cell: 0
            })
        );
        assert_eq!(
            TranscriptScroll::anchor_for(&meta, 3),
            Some(TranscriptScroll::Scrolled {
                cell_index: 1,
                line_in_cell: 0
            })
        );
    }
}
