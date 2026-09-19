//! Carry reading anchors through commits and retain displayed or searched groups across page joins.

use std::ops::Range;

use super::*;

impl TranscriptView {
    pub(crate) fn history_loaded(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        inserted: Range<usize>,
    ) {
        // With only a header and live output, the first older page extends the canonical tail.
        if !inserted.is_empty()
            && inserted.end == cells.len()
            && self.last_tail == cells[..inserted.start].last().map(EntryKey::cell)
        {
            self.last_tail = cells.last().map(EntryKey::cell);
        }
        self.prepend_snapshot_history(cells, inserted);
        if let Position::Reading(anchor) = self.position {
            let index = self.resolve(cells, anchor);
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                ..anchor
            });
        }
    }

    /// Refresh unseen activity before either footer measurement or transcript painting.
    pub(crate) fn sync_history_tail(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        let tail = cells.last().map(EntryKey::cell);
        if !self.is_following() && self.last_tail.is_some() && self.last_tail != tail {
            let previous = cells
                .iter()
                .rposition(|cell| Some(EntryKey::cell(cell)) == self.last_tail);
            self.unseen_activity |= previous.is_none_or(|index| {
                (index + 1..cells.len()).any(|index| {
                    self.current_layout(cells, index)
                        .is_some_and(|layout| layout.row_count() > 0)
                })
            });
        }
        self.last_tail = tail;
        if self.is_following() {
            self.unseen_activity = false;
        }
    }

    /// Replacements restart search and move affected readers; held readers and selections keep their snapshot.
    pub(crate) fn replace_range(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        range: Range<usize>,
        replacement: &Arc<dyn HistoryCell>,
    ) {
        if range.is_empty() {
            return;
        }
        if range.end == cells.len() && self.last_tail == cells.last().map(EntryKey::cell) {
            self.last_tail = Some(EntryKey::cell(replacement));
        }
        if self.snapshot().is_some() {
            return;
        }
        let reading = match self.position {
            Position::Reading(anchor) if range.contains(&self.resolve(cells, anchor)) => {
                Some(anchor)
            }
            _ => None,
        };
        if reading.is_none() {
            return;
        }
        let layouts = range
            .clone()
            .filter_map(|index| self.layout(cells, index))
            .collect::<Vec<_>>();
        let replacement_key = EntryKey::cell(replacement);
        if let Some(anchor) = reading {
            self.position = Position::Reading(remap(
                anchor,
                self.resolve(cells, anchor),
                range.start,
                replacement_key,
                &layouts,
            ));
        }
    }

    /// Join unseen groups while retaining displayed groups and the search frontier.
    ///
    /// Call after inserting the older page and before replacing the old group cells. Unlike
    /// streamed text finalization, group summaries cannot preserve offsets by concatenation.
    /// Search keeps both sides of the join so it can finish scanning the newly inserted page.
    pub(crate) fn replace_group(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        range: Range<usize>,
        replacement: &Arc<dyn HistoryCell>,
    ) {
        if range.is_empty() {
            return;
        }
        // Older history may extend the final group without introducing new activity.
        if range.end == cells.len() && self.last_tail == cells.last().map(EntryKey::cell) {
            self.last_tail = Some(EntryKey::cell(replacement));
        }
        let reading = match self.position {
            Position::Reading(anchor) => Some(anchor.key),
            Position::Latest => None,
        };
        let visible = self.visible.iter().any(|row| {
            cells[range.clone()]
                .iter()
                .any(|cell| EntryKey::cell(cell) == row.key)
        });
        if let Some(snapshot) = self.snapshot_mut() {
            let replaced = &cells[range];
            // Pins retain the initial viewport and selected text; later scrolling adds no pins.
            if visible
                || replaced.iter().any(|cell| {
                    let key = EntryKey::cell(cell);
                    snapshot.pinned.contains_key(&key) || reading == Some(key)
                })
            {
                return;
            }
            let Some(start) = snapshot.cells.windows(replaced.len()).position(|retained| {
                retained
                    .iter()
                    .zip(replaced)
                    .all(|(a, b)| Arc::ptr_eq(a, b))
            }) else {
                return;
            };
            let mut joined = snapshot.cells.to_vec();
            joined.splice(start..start + replaced.len(), [Arc::clone(replacement)]);
            snapshot.cells = joined.into();
            return;
        }
        if matches!(self.position, Position::Reading(anchor)
                if cells[range].iter().any(|cell| EntryKey::cell(cell) == anchor.key))
        {
            self.held_reading = Some(self.capture_snapshot(cells));
        }
    }

    /// Retain displayed revisions when an older completed group joins the active group.
    ///
    /// Call after `history_loaded` and the live mutation, before removing the canonical tail.
    /// The widget revisions bracket only hydration so actual pending activity remains visible.
    pub(crate) fn absorb_tail_into_live(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        previous_revision: u64,
        hydrated_revision: u64,
    ) {
        let Some((tail, preceding)) = cells.split_last() else {
            return;
        };
        let tail_key = EntryKey::cell(tail);
        if self.last_tail == Some(tail_key) {
            self.last_tail = preceding.last().map(EntryKey::cell);
        }
        let following = self.is_following();
        if let Some((_, key)) = &mut self.live_key {
            if !following && key.revision != previous_revision {
                self.unseen_activity = true;
            }
            key.revision = hydrated_revision;
            // Force fresh lines even if the next draw receives this exact hydrated revision.
            key.cacheable = false;
        }
        if self.snapshot().is_some() {
            return;
        }
        if let Position::Reading(anchor) = self.position
            && (anchor.key == tail_key || anchor.key == EntryKey::Live)
        {
            self.held_reading = Some(self.capture_snapshot(cells));
        }
    }

    /// Extend the frozen history only with the explicitly inserted older page.
    ///
    /// Current commits and replacements remain outside the snapshot. A retained session header
    /// identifies the insertion boundary even when it precedes the first history page.
    pub(super) fn prepend_snapshot_history(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        inserted: Range<usize>,
    ) {
        let Some(snapshot) = self.snapshot_mut() else {
            return;
        };
        let existing = snapshot
            .cells
            .iter()
            .map(EntryKey::cell)
            .collect::<std::collections::HashSet<_>>();
        let older = cells[inserted.clone()]
            .iter()
            .filter(|cell| !existing.contains(&EntryKey::cell(cell)))
            .cloned()
            .collect::<Vec<_>>();
        if older.is_empty() {
            return;
        }
        let index = cells[..inserted.start]
            .iter()
            .rev()
            .find_map(|cell| {
                snapshot
                    .cells
                    .iter()
                    .position(|retained| Arc::ptr_eq(retained, cell))
            })
            .map_or(/*default*/ 0, |index| index + 1);
        let mut extended = snapshot.cells.to_vec();
        extended.splice(index..index, older);
        snapshot.cells = extended.into();
    }
}

#[cfg(test)]
#[path = "mutations_tests.rs"]
mod tests;

fn remap(
    anchor: Anchor,
    index: usize,
    first: usize,
    key: EntryKey,
    layouts: &[Arc<TextLayout>],
) -> Anchor {
    let preceding = layouts
        .iter()
        .enumerate()
        .take(index - first)
        .filter(|(_, layout)| layout.row_count() > 0)
        .map(|(index, layout)| {
            layout.text().len()
                + layouts[index + 1..]
                    .iter()
                    .find(|next| next.row_count() > 0)
                    .map_or(0, |next| layout.separator_after(next).len())
        })
        .sum::<usize>();
    Anchor {
        key,
        index: first,
        offset: preceding.saturating_add(anchor.offset),
        row_bias: anchor.row_bias,
    }
}
