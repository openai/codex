//! Retain displayed revisions while selecting or reading a live tail or a replaced tool group.

use std::collections::HashMap;

use super::*;

#[derive(Clone)]
pub(super) struct ViewSnapshot {
    pub(super) cells: Arc<[Arc<dyn HistoryCell>]>,
    pub(super) pinned: HashMap<EntryKey, Arc<TextLayout>>,
    pub(super) activities: HashMap<EntryKey, Arc<[String]>>,
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
        let mut activities = self
            .snapshot()
            .map(|snapshot| snapshot.activities.clone())
            .unwrap_or_default();
        pinned.extend(
            self.visible
                .iter()
                .map(|visible| (visible.key, Arc::clone(&visible.layout))),
        );
        activities.extend(
            self.visible
                .iter()
                .map(|visible| (visible.key, Arc::clone(&visible.activity_ids))),
        );
        if let Some(live) = self.layout(&cells, cells.len()) {
            pinned.insert(EntryKey::Live, live);
            activities.insert(
                EntryKey::Live,
                self.displayed_activity_ids(&cells, cells.len()),
            );
        }
        ViewSnapshot {
            cells,
            pinned,
            activities,
        }
    }

    /// Keep a live, mutable, or replaced cell revision while reading inside it. Navigation
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
        let mutable_cell = cells.get(anchor.index).is_some_and(|cell| {
            EntryKey::cell(cell) == anchor.key
                && (!cell.has_stable_transcript_height()
                    || cell.transcript_animation_tick().is_some())
        });
        if anchor.key == EntryKey::Live || mutable_cell {
            if self
                .held_reading
                .as_ref()
                .is_none_or(|snapshot| !snapshot.pinned.contains_key(&anchor.key))
            {
                self.held_reading = None;
                let mut snapshot = self.capture_snapshot(cells);
                snapshot.pinned.insert(anchor.key, layout);
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
        if self.held_reading.take().is_some() {
            self.invalidate_held_search();
        }
    }

    pub(super) fn rewrap_snapshot(&mut self, width: u16) {
        if let Some(snapshot) = self.snapshot_mut() {
            for layout in snapshot.pinned.values_mut() {
                *layout = Arc::new(layout.rewrap(width));
            }
        }
    }
}
