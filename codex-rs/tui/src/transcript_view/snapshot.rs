//! Retain displayed revisions while selecting or reading a live tail or a replaced tool group.

use std::collections::HashMap;

use super::*;

#[derive(Clone)]
pub(super) struct ViewSnapshot {
    pub(super) cells: Arc<[Arc<dyn HistoryCell>]>,
    pub(super) pinned: HashMap<EntryKey, Arc<TextLayout>>,
}

impl TranscriptView {
    pub(super) fn snapshot(&self) -> Option<&ViewSnapshot> {
        self.selection
            .as_ref()
            .map(|selection| &selection.snapshot)
            .or(self.held_reading.as_ref())
    }

    pub(super) fn snapshot_mut(&mut self) -> Option<&mut ViewSnapshot> {
        self.selection
            .as_mut()
            .map(|selection| &mut selection.snapshot)
            .or(self.held_reading.as_mut())
    }

    /// Clone the handle before mutating view state, without copying cells per frame.
    pub(super) fn snapshot_cells(&self) -> Option<Arc<[Arc<dyn HistoryCell>]>> {
        self.snapshot().map(|snapshot| Arc::clone(&snapshot.cells))
    }

    pub(super) fn capture_snapshot(&mut self, cells: &[Arc<dyn HistoryCell>]) -> ViewSnapshot {
        let cells = self.snapshot_cells().unwrap_or_else(|| Arc::from(cells));
        let mut pinned = self
            .snapshot()
            .map(|snapshot| snapshot.pinned.clone())
            .unwrap_or_default();
        pinned.extend(
            self.visible
                .iter()
                .map(|visible| (visible.key, Arc::clone(&visible.layout))),
        );
        if let Some(live) = self.layout(&cells, cells.len()) {
            pinned.insert(EntryKey::Live, live);
        }
        ViewSnapshot { cells, pinned }
    }

    /// Keep a live or replaced group's revision while reading inside it. Ordinary navigation
    /// rejoins current history when it reaches a surviving cell.
    pub(super) fn hold_live_reading(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        layout: Arc<TextLayout>,
    ) {
        if self.selection.is_some() {
            return;
        }
        let Position::Reading(anchor) = self.position else {
            self.release_live_reading();
            return;
        };
        if anchor.key == EntryKey::Live {
            if self.held_reading.is_none() {
                let mut snapshot = self.capture_snapshot(cells);
                snapshot.pinned.insert(EntryKey::Live, layout);
                self.held_reading = Some(snapshot);
            }
        } else if self.held_reading.is_some()
            && let Some(index) = cells
                .iter()
                .position(|cell| EntryKey::cell(cell) == anchor.key)
        {
            self.release_live_reading();
            self.position = Position::Reading(Anchor { index, ..anchor });
        }
    }

    /// Search offsets belong to the held revision and must be refreshed when it is released.
    pub(super) fn release_live_reading(&mut self) {
        self.held_reading = None;
    }

    pub(super) fn rewrap_snapshot(&mut self, width: u16) {
        if let Some(snapshot) = self.snapshot_mut() {
            for layout in snapshot.pinned.values_mut() {
                *layout = Arc::new(layout.rewrap(width));
            }
        }
    }
}
