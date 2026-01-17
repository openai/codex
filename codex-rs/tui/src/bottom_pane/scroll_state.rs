//! Scroll and selection bookkeeping for bottom-pane list popups.
//!
//! This module provides a tiny state machine used by selection lists to track
//! the highlighted row and the first visible index of the scroll window. It
//! encapsulates wrap-around navigation and the logic to keep the selected row
//! within the currently visible range.
//!
//! The state is intentionally generic and does not know about item content or
//! rendering; callers must supply the list length and visible row count.
//!
//! Callers are expected to clamp selection whenever the list length changes and
//! to call [`ScrollState::ensure_visible`] after navigation so `scroll_top`
//! tracks the highlighted row.

/// Scroll and selection state for a vertical list menu.
///
/// The state tracks the selected index and the first visible row in the list
/// viewport. Callers are responsible for keeping it in sync with their item
/// collection and the number of visible rows they can render. Navigation methods
/// only update the selection; callers must invoke [`ScrollState::ensure_visible`]
/// to keep the scroll window aligned.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ScrollState {
    /// Currently selected row in the full list, or `None` when the list is empty.
    pub selected_idx: Option<usize>,
    /// Index of the first visible row in the list viewport.
    ///
    /// When the list is non-empty, this is expected to stay within `0..len`.
    pub scroll_top: usize,
}

impl ScrollState {
    /// Creates a fresh state with no selection and a zero scroll offset.
    ///
    /// Call [`ScrollState::clamp_selection`] once the list has items to set an
    /// initial selection.
    pub fn new() -> Self {
        Self {
            selected_idx: None,
            scroll_top: 0,
        }
    }

    /// Resets selection and scroll position back to the initial state.
    pub fn reset(&mut self) {
        self.selected_idx = None;
        self.scroll_top = 0;
    }

    /// Clamps selection to the valid range for a list of the given length.
    ///
    /// If the list is empty, the selection is cleared and the scroll offset is
    /// reset to zero.
    pub fn clamp_selection(&mut self, len: usize) {
        self.selected_idx = match len {
            0 => None,
            _ => Some(self.selected_idx.unwrap_or(0).min(len - 1)),
        };
        if len == 0 {
            self.scroll_top = 0;
        }
    }

    /// Moves selection up by one row, wrapping to the bottom when needed.
    ///
    /// If the list is empty, the selection is cleared and the scroll offset is
    /// reset to zero. This does not adjust `scroll_top`; call
    /// [`ScrollState::ensure_visible`] after moving.
    pub fn move_up_wrap(&mut self, len: usize) {
        if len == 0 {
            self.selected_idx = None;
            self.scroll_top = 0;
            return;
        }
        self.selected_idx = Some(match self.selected_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) => len - 1,
            None => 0,
        });
    }

    /// Moves selection down by one row, wrapping to the top when needed.
    ///
    /// If the list is empty, the selection is cleared and the scroll offset is
    /// reset to zero. This does not adjust `scroll_top`; call
    /// [`ScrollState::ensure_visible`] after moving.
    pub fn move_down_wrap(&mut self, len: usize) {
        if len == 0 {
            self.selected_idx = None;
            self.scroll_top = 0;
            return;
        }
        self.selected_idx = Some(match self.selected_idx {
            Some(idx) if idx + 1 < len => idx + 1,
            _ => 0,
        });
    }

    /// Adjusts `scroll_top` so the selected row stays within the visible window.
    ///
    /// The caller supplies the total list length and number of visible rows; if
    /// either is zero, the scroll position resets to zero.
    pub fn ensure_visible(&mut self, len: usize, visible_rows: usize) {
        if len == 0 || visible_rows == 0 {
            self.scroll_top = 0;
            return;
        }
        if let Some(sel) = self.selected_idx {
            if sel < self.scroll_top {
                self.scroll_top = sel;
            } else {
                let bottom = self.scroll_top + visible_rows - 1;
                if sel > bottom {
                    self.scroll_top = sel + 1 - visible_rows;
                }
            }
        } else {
            self.scroll_top = 0;
        }
    }
}

/// Snapshot-style tests for wrap-around navigation and visibility tracking.
#[cfg(test)]
mod tests {
    use super::ScrollState;
    use pretty_assertions::assert_eq;

    /// Drives the selection past each edge and asserts it remains visible.
    #[test]
    fn wrap_navigation_and_visibility() {
        let mut s = ScrollState::new();
        let len = 10;
        let vis = 5;

        s.clamp_selection(len);
        assert_eq!(s.selected_idx, Some(0));
        s.ensure_visible(len, vis);
        assert_eq!(s.scroll_top, 0);

        s.move_up_wrap(len);
        s.ensure_visible(len, vis);
        assert_eq!(s.selected_idx, Some(len - 1));
        match s.selected_idx {
            Some(sel) => assert!(s.scroll_top <= sel),
            None => panic!("expected Some(selected_idx) after wrap"),
        }

        s.move_down_wrap(len);
        s.ensure_visible(len, vis);
        assert_eq!(s.selected_idx, Some(0));
        assert_eq!(s.scroll_top, 0);
    }
}
