/// Generic scroll/selection state for a vertical list menu.
///
/// Encapsulates the common behavior of a selectable list that supports:
/// - Optional selection (None when list is empty)
/// - Wrap-around navigation on Up/Down
/// - Maintaining a scroll window (`scroll_top`) so the selected row stays visible
///
/// Callers own the filtered row count and the visible window size. Every
/// mutation method takes those values instead of caching them here, so list
/// views can apply filters, pagination, or density changes without this helper
/// knowing about their data model. Passing a stale length after filtering would
/// leave selection pointing at the wrong row, so callers should clamp or move
/// through this type immediately after changing their visible row set.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ScrollState {
    pub selected_idx: Option<usize>,
    pub scroll_top: usize,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            selected_idx: None,
            scroll_top: 0,
        }
    }

    /// Reset selection and scroll.
    pub fn reset(&mut self) {
        self.selected_idx = None;
        self.scroll_top = 0;
    }

    /// Clamp selection to be within the [0, len-1] range, or None when empty.
    pub fn clamp_selection(&mut self, len: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        self.selected_idx = Some(self.selected_idx.unwrap_or(0).min(len - 1));
    }

    /// Move selection up by one, wrapping to the bottom when necessary.
    pub fn move_up_wrap(&mut self, len: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        self.selected_idx = Some(match self.selected_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) => len - 1,
            None => 0,
        });
    }

    /// Move selection down by one, wrapping to the top when necessary.
    pub fn move_down_wrap(&mut self, len: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        self.selected_idx = Some(match self.selected_idx {
            Some(idx) if idx + 1 < len => idx + 1,
            _ => 0,
        });
    }

    /// Move selection up by one visible page, clamping at the first row.
    ///
    /// Page movement intentionally does not wrap. It mirrors terminal list
    /// behavior where repeated page-up/page-down converges at the nearest edge
    /// while still keeping the selected row visible.
    pub fn page_up_clamped(&mut self, len: usize, visible_rows: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        let step = visible_rows.max(1);
        let current = self.selected_idx.unwrap_or(0).min(len - 1);
        self.selected_idx = Some(current.saturating_sub(step));
        self.ensure_visible(len, visible_rows);
    }

    /// Move selection down by one visible page, clamping at the last row.
    ///
    /// Page movement intentionally does not wrap. It mirrors terminal list
    /// behavior where repeated page-up/page-down converges at the nearest edge
    /// while still keeping the selected row visible.
    pub fn page_down_clamped(&mut self, len: usize, visible_rows: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        let step = visible_rows.max(1);
        let current = self.selected_idx.unwrap_or(0).min(len - 1);
        self.selected_idx = Some(current.saturating_add(step).min(len - 1));
        self.ensure_visible(len, visible_rows);
    }

    /// Jump selection to the first row.
    pub fn jump_top(&mut self, len: usize, visible_rows: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        self.selected_idx = Some(0);
        self.ensure_visible(len, visible_rows);
    }

    /// Jump selection to the last row.
    pub fn jump_bottom(&mut self, len: usize, visible_rows: usize) {
        if self.clear_if_empty(len) {
            return;
        }
        self.selected_idx = Some(len - 1);
        self.ensure_visible(len, visible_rows);
    }

    fn clear_if_empty(&mut self, len: usize) -> bool {
        if len != 0 {
            return false;
        }
        self.selected_idx = None;
        self.scroll_top = 0;
        true
    }

    /// Adjust `scroll_top` so that the current `selected_idx` is visible within
    /// the window of `visible_rows`.
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

#[cfg(test)]
mod tests {
    use super::ScrollState;

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

    #[test]
    fn page_and_jump_navigation_clamps() {
        let mut s = ScrollState::new();
        let len = 10;
        let vis = 4;

        s.clamp_selection(len);
        s.page_down_clamped(len, vis);
        assert_eq!(s.selected_idx, Some(4));
        assert_eq!(s.scroll_top, 1);

        s.page_down_clamped(len, vis);
        assert_eq!(s.selected_idx, Some(8));
        assert_eq!(s.scroll_top, 5);

        s.page_down_clamped(len, vis);
        assert_eq!(s.selected_idx, Some(9));
        assert_eq!(s.scroll_top, 6);

        s.page_up_clamped(len, vis);
        assert_eq!(s.selected_idx, Some(5));
        assert_eq!(s.scroll_top, 5);

        s.jump_top(len, vis);
        assert_eq!(s.selected_idx, Some(0));
        assert_eq!(s.scroll_top, 0);

        s.jump_bottom(len, vis);
        assert_eq!(s.selected_idx, Some(9));
        assert_eq!(s.scroll_top, 6);
    }
}
