/// Scroll state for the inline transcript viewport.
///
/// This tracks whether the transcript is pinned to the latest line or anchored
/// at a specific cell/line pair so later viewport changes can implement
/// scrollback without losing the notion of "bottom".
#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum TranscriptScroll {
    #[default]
    ToBottom,
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
    /// for spacer rows between cells.
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
    /// See `resolve_top` for `meta` semantics.
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
    /// See `resolve_top` for `meta` semantics.
    pub(crate) fn anchor_for(meta: &[Option<(usize, usize)>], start: usize) -> Option<Self> {
        let anchor = anchor_at_or_after(meta, start).or_else(|| anchor_at_or_before(meta, start));
        anchor.map(|(cell_index, line_in_cell)| Self::Scrolled {
            cell_index,
            line_in_cell,
        })
    }
}

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

fn anchor_at_or_after(meta: &[Option<(usize, usize)>], start: usize) -> Option<(usize, usize)> {
    if meta.is_empty() {
        return None;
    }
    let start = start.min(meta.len().saturating_sub(1));
    meta.iter().skip(start).flatten().next().copied()
}

fn anchor_at_or_before(meta: &[Option<(usize, usize)>], start: usize) -> Option<(usize, usize)> {
    if meta.is_empty() {
        return None;
    }
    let start = start.min(meta.len().saturating_sub(1));
    meta[..=start].iter().rev().flatten().next().copied()
}
