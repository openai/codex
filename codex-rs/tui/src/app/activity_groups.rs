//! Repair activity groups split by transport pages without rewriting terminal scrollback.
//!
//! Only persisted member adjacency within one turn permits joins. Overlay callbacks retain the
//! displayed source revision before canonical cells are replaced or absorbed into the live tail.

use super::*;
use crate::thread_transcript::join_computer_groups;
use crate::thread_transcript::join_exploration_groups;

impl App {
    pub(super) fn join_older_activity_group(&mut self, boundary: usize, turns: &[Turn]) {
        let Some(start) = self.transcript_cells[..boundary]
            .iter()
            .rposition(|cell| !crate::thread_transcript::is_hidden_activity_detail(cell))
        else {
            return;
        };
        let mut newer_index = self.transcript_cells[boundary..]
            .iter()
            .position(|cell| !crate::thread_transcript::is_hidden_activity_detail(cell))
            .map_or(self.transcript_cells.len(), |offset| boundary + offset);
        // A reasoning-only page belongs to the preceding group even when an answer or turn
        // separator follows it. Verify identities before touching either side of that boundary.
        if start + 1 < newer_index {
            let Some(older) = crate::thread_transcript::fold_trailing_activity_details(
                &self.transcript_cells[start],
                &self.transcript_cells[start + 1..newer_index],
                turns,
            ) else {
                return;
            };
            let range = start..newer_index;
            self.native_history
                .consolidate(&self.transcript_cells[range.clone()], &older);
            self.transcript_view
                .replace_group(&self.transcript_cells, range.clone(), &older);
            if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
                overlay.regroup_cells(range.clone(), Arc::clone(&older));
            }
            self.transcript_cells.splice(range, [older]);
            newer_index = start + 1;
        }
        if newer_index == self.transcript_cells.len() {
            let Some((previous_revision, hydrated_revision)) = self
                .chat_widget
                .prepend_active_computer_history(self.transcript_cells[start].as_ref(), turns)
                .or_else(|| {
                    self.chat_widget.prepend_active_exploration_history(
                        self.transcript_cells[start].as_ref(),
                        turns,
                    )
                })
            else {
                return;
            };
            self.transcript_view.absorb_tail_into_live(
                &self.transcript_cells,
                previous_revision,
                hydrated_revision,
            );
            if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
                overlay.absorb_tail_into_live(previous_revision, hydrated_revision);
            }
            self.transcript_cells.pop();
            self.native_history.retain(&self.transcript_cells);
            return;
        }
        let Some(newer) = self.transcript_cells.get(newer_index) else {
            return;
        };
        let Some(group) = join_computer_groups(&self.transcript_cells[start], newer, turns)
            .or_else(|| join_exploration_groups(&self.transcript_cells[start], newer, turns))
        else {
            return;
        };
        let range = start..newer_index + 1;
        self.native_history
            .consolidate(&self.transcript_cells[range.clone()], &group);
        self.transcript_view
            .replace_group(&self.transcript_cells, range.clone(), &group);
        if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
            overlay.regroup_cells(range.clone(), Arc::clone(&group));
        }
        self.transcript_cells.splice(range, [group]);
    }
}

#[cfg(test)]
#[path = "activity_groups_tests.rs"]
mod tests;
