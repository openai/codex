//! Queue native terminal output behind unfinished dynamic tools.
//! Fullscreen rendering reads retained cells directly and never queues terminal output.
//!
//! Entries own their render source independently of the retained view. Consolidation remaps their
//! canonical owner, so pagination, pointer reuse, and replacing stream fragments cannot cause a
//! second emission. Clearing the transcript starts a new journal; rollback prunes removed owners.

use super::*;

const CELLS_PER_FRAME: usize = 32;

struct PendingCell {
    source: Arc<dyn HistoryCell>,
    owner: std::sync::Weak<dyn HistoryCell>,
}

#[derive(Default)]
pub(super) struct NativeHistory {
    pending: VecDeque<PendingCell>,
}

pub(super) fn is_pending(cell: &dyn HistoryCell) -> bool {
    cell.as_any()
        .downcast_ref::<history_cell::DynamicToolCallCell>()
        .is_some_and(history_cell::DynamicToolCallCell::is_active)
}

impl NativeHistory {
    pub(super) fn contains(&self, cell: &Arc<dyn HistoryCell>) -> bool {
        self.pending
            .iter()
            .any(|entry| Arc::ptr_eq(&entry.source, cell))
    }

    pub(super) fn insert(&mut self, cell: &Arc<dyn HistoryCell>) -> bool {
        if is_pending(cell.as_ref()) || !self.pending.is_empty() {
            self.defer(cell);
            true
        } else {
            false
        }
    }

    pub(super) fn defer(&mut self, cell: &Arc<dyn HistoryCell>) {
        self.pending.push_back(PendingCell {
            source: cell.clone(),
            owner: Arc::downgrade(cell),
        });
    }

    /// Replace an entirely unprinted stream with final source. For a partly printed stream, keep
    /// only its remaining fragments; never replay the prefix already owned by the terminal.
    pub(super) fn consolidate(
        &mut self,
        previous: &[Arc<dyn HistoryCell>],
        consolidated: &Arc<dyn HistoryCell>,
    ) {
        let belongs = |entry: &PendingCell| {
            previous
                .iter()
                .any(|cell| entry.owner.ptr_eq(&Arc::downgrade(cell)))
        };
        let all_pending = !previous.is_empty() && previous.iter().all(|cell| self.contains(cell));
        if all_pending && let Some(index) = self.pending.iter().position(&belongs) {
            self.pending.retain(|entry| !belongs(entry));
            // Keep consolidated output before unrelated cells that were queued after it.
            self.pending.insert(
                index,
                PendingCell {
                    source: consolidated.clone(),
                    owner: Arc::downgrade(consolidated),
                },
            );
        } else {
            for entry in self.pending.iter_mut().filter(|entry| belongs(entry)) {
                entry.owner = Arc::downgrade(consolidated);
            }
        }
    }

    pub(super) fn retain(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        self.pending.retain(|entry| {
            cells
                .iter()
                .any(|cell| entry.owner.ptr_eq(&Arc::downgrade(cell)))
        });
    }

    /// Historical in-progress records are not live blockers: only queued owners can hold replay.
    pub(super) fn replayable_cells<'a>(
        &self,
        cells: &'a [Arc<dyn HistoryCell>],
    ) -> &'a [Arc<dyn HistoryCell>] {
        let end = self
            .pending
            .iter()
            .find(|entry| entry.owner.strong_count() > 0 && is_pending(entry.source.as_ref()))
            .and_then(|entry| {
                cells
                    .iter()
                    .position(|cell| entry.owner.ptr_eq(&Arc::downgrade(cell)))
            })
            .unwrap_or(cells.len());
        &cells[..end]
    }

    /// A deliberate native resize/rollback replay supersedes queued settled output.
    pub(super) fn replayed(&mut self) {
        let mut blocked = false;
        self.pending.retain(|entry| {
            blocked |= is_pending(entry.source.as_ref());
            blocked
        });
    }
}

impl App {
    pub(super) fn flush_native_history(&mut self, tui: &mut tui::Tui) {
        if self.overlay.is_some() {
            return;
        }
        let mut ready = Vec::new();
        while let Some(entry) = self.native_history.pending.front() {
            if entry.owner.strong_count() == 0 {
                self.native_history.pending.pop_front();
                continue;
            }
            if ready.len() == CELLS_PER_FRAME || is_pending(entry.source.as_ref()) {
                break;
            }
            ready.push(Arc::clone(&entry.source));
            self.native_history.pending.pop_front();
        }
        for cell in ready {
            self.render_inserted_history_cell(tui, &cell, /*deferred*/ false);
        }
        if self
            .native_history
            .pending
            .front()
            .is_some_and(|entry| !is_pending(entry.source.as_ref()))
        {
            tui.frame_requester().schedule_frame();
        }
    }
}

#[cfg(test)]
#[path = "native_history_tests.rs"]
mod tests;
