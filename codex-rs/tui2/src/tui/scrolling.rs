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
    Scrolled {
        cell_index: usize,
        line_in_cell: usize,
    },
}

impl TranscriptScroll {
    /// Resolve the top row for the current scroll state.
    ///
    /// `meta` is a line-parallel mapping of flattened transcript lines where each entry is
    /// `Some((cell_index, line_in_cell))` for a line emitted by a history cell or `None`
    /// for spacer rows between cells. Returns the resolved scroll state plus the top row
    /// offset, clamping to `max_start` if the anchor moved beyond the current window.
    pub(crate) fn resolve_top(
        self,
        meta: &[Option<(usize, usize)>],
        max_start: usize,
    ) -> (Self, usize) {
        match self {
            Self::ToBottom => (Self::ToBottom, max_start),
            Self::Scrolled {
                cell_index,
                line_in_cell,
            } => {
                let anchor = anchor_index(meta, cell_index, line_in_cell);
                match anchor {
                    Some(idx) => (self, idx.min(max_start)),
                    None => (Self::ToBottom, max_start),
                }
            }
        }
    }

    /// Apply a scroll delta and return the updated scroll state.
    ///
    /// See `resolve_top` for `meta` semantics. Positive deltas scroll toward the latest
    /// transcript content, while negative deltas move upward into scrollback.
    pub(crate) fn scrolled_by(
        self,
        delta_lines: i32,
        meta: &[Option<(usize, usize)>],
        visible_lines: usize,
    ) -> Self {
        if delta_lines == 0 {
            return self;
        }

        let total_lines = meta.len();
        if total_lines <= visible_lines {
            return Self::ToBottom;
        }

        let max_start = total_lines.saturating_sub(visible_lines);
        let current_top = match self {
            Self::ToBottom => max_start,
            Self::Scrolled {
                cell_index,
                line_in_cell,
            } => anchor_index(meta, cell_index, line_in_cell)
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

        Self::anchor_for(meta, new_top).unwrap_or(Self::ToBottom)
    }

    /// Anchor to the first available line at or near the given start offset.
    ///
    /// See `resolve_top` for `meta` semantics. This prefers the nearest line at or after
    /// `start`, falling back to the nearest line before it when needed.
    pub(crate) fn anchor_for(meta: &[Option<(usize, usize)>], start: usize) -> Option<Self> {
        let anchor = anchor_at_or_after(meta, start).or_else(|| anchor_at_or_before(meta, start));
        anchor.map(|(cell_index, line_in_cell)| Self::Scrolled {
            cell_index,
            line_in_cell,
        })
    }
}

/// Locate the flattened line index for a specific transcript cell and line.
fn anchor_index(
    meta: &[Option<(usize, usize)>],
    cell_index: usize,
    line_in_cell: usize,
) -> Option<usize> {
    meta.iter()
        .enumerate()
        .find_map(|(idx, entry)| match entry {
            Some((ci, li)) if *ci == cell_index && *li == line_in_cell => Some(idx),
            _ => None,
        })
}

/// Find the first transcript line at or after the given flattened index.
fn anchor_at_or_after(meta: &[Option<(usize, usize)>], start: usize) -> Option<(usize, usize)> {
    if meta.is_empty() {
        return None;
    }
    let start = start.min(meta.len().saturating_sub(1));
    meta.iter().skip(start).flatten().next().copied()
}

/// Find the nearest transcript line at or before the given flattened index.
fn anchor_at_or_before(meta: &[Option<(usize, usize)>], start: usize) -> Option<(usize, usize)> {
    if meta.is_empty() {
        return None;
    }
    let start = start.min(meta.len().saturating_sub(1));
    meta[..=start].iter().rev().flatten().next().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn meta(entries: &[Option<(usize, usize)>]) -> Vec<Option<(usize, usize)>> {
        entries.to_vec()
    }

    #[test]
    fn resolve_top_to_bottom_clamps_to_max_start() {
        let meta = meta(&[Some((0, 0)), Some((0, 1)), None, Some((1, 0))]);

        let (state, top) = TranscriptScroll::ToBottom.resolve_top(&meta, 3);

        assert_eq!(state, TranscriptScroll::ToBottom);
        assert_eq!(top, 3);
    }

    #[test]
    fn resolve_top_scrolled_keeps_anchor_when_present() {
        let meta = meta(&[Some((0, 0)), None, Some((1, 0)), Some((1, 1))]);
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
        let meta = meta(&[Some((0, 0)), None, Some((1, 0))]);
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
            Some((0, 0)),
            Some((0, 1)),
            Some((1, 0)),
            None,
            Some((2, 0)),
            Some((2, 1)),
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
        let meta = meta(&[Some((0, 0)), Some((0, 1)), Some((1, 0)), Some((2, 0))]);
        let scroll = TranscriptScroll::Scrolled {
            cell_index: 0,
            line_in_cell: 0,
        };

        let state = scroll.scrolled_by(5, &meta, 2);

        assert_eq!(state, TranscriptScroll::ToBottom);
    }

    #[test]
    fn scrolled_by_to_bottom_when_all_lines_fit() {
        let meta = meta(&[Some((0, 0)), Some((0, 1))]);

        let state = TranscriptScroll::Scrolled {
            cell_index: 0,
            line_in_cell: 0,
        }
        .scrolled_by(-1, &meta, 5);

        assert_eq!(state, TranscriptScroll::ToBottom);
    }

    #[test]
    fn anchor_for_prefers_after_then_before() {
        let meta = meta(&[None, Some((0, 0)), None, Some((1, 0))]);

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
