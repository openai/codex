//! A tail update is unseen only until its current final row has been painted.
//! Retained selections and reading snapshots may still hide a newer revision at the bottom.

use super::*;

impl TranscriptView {
    pub(crate) fn can_return_to_latest(&self) -> bool {
        !self.is_following()
    }

    pub(super) fn current_tail_is_visible(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        // Empty live previews and hidden reasoning cells do not obscure the actual tail.
        for index in (0..=cells.len()).rev() {
            let Some(layout) = self.current_layout(cells, index) else {
                continue;
            };
            if layout.row_count() == 0 {
                continue;
            }
            let key = cells.get(index).map_or(EntryKey::Live, EntryKey::cell);
            return self.visible.last().is_some_and(|visible| {
                visible.key == key
                    && visible.layout.text() == layout.text()
                    && visible.row + 1 == layout.row_count()
            });
        }
        true
    }
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
