//! Update committed command activity and join groups split by transport pages.
//!
//! Only persisted member adjacency within one turn permits joins. Overlay callbacks retain the
//! displayed source revision before canonical cells are replaced or absorbed into the live tail.

use super::*;
use crate::exec_cell::ExecCell;
use crate::thread_transcript::join_computer_groups;
use crate::thread_transcript::join_exploration_groups;

/// Locate an unfinished call and preserve its neighbors and transcript details.
pub(super) fn pending_command_group(
    cells: &[Arc<dyn HistoryCell>],
    id: &str,
) -> Option<(usize, usize, ExecCell)> {
    cells.iter().enumerate().rev().find_map(|(index, cell)| {
        let group = cell.as_any().downcast_ref::<ExecCell>()?;
        let member = group
            .iter_calls()
            .position(|call| call.call_id == id && call.duration.is_none())?;
        Some((index, member, group.clone()))
    })
}

impl App {
    pub(crate) fn complete_background_command(
        &mut self,
        tui: &mut tui::Tui,
        mut completed: Box<ExecCell>,
    ) {
        let Some((index, member, mut group)) =
            pending_command_group(&self.transcript_cells, &completed.group.calls[0].call_id)
        else {
            // The transcript may have been cleared while the command was running.
            self.insert_history_cell(tui, completed);
            return;
        };
        // Consolidation replaces an unprinted launch. Otherwise append just this result.
        let owner = &self.transcript_cells[index];
        if !tui.is_owned_screen()
            && !self.native_history.contains(owner)
            && !self.native_history.defer_command_result(owner, &completed)
        {
            let width = self
                .chat_widget
                .history_wrap_width(tui.terminal.last_known_screen_size.width);
            self.insert_history_cell_lines(tui, completed.as_ref(), width);
            self.last_rendered_history_tail = None;
        }
        group.group.calls[member] = completed.group.calls.remove(0);
        self.replace_activity_group(index..index + 1, Arc::new(group));
        tui.frame_requester().schedule_frame();
    }

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
            self.replace_activity_group(start..newer_index, older);
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
        self.replace_activity_group(start..newer_index + 1, group);
    }

    fn replace_activity_group(
        &mut self,
        range: std::ops::Range<usize>,
        group: Arc<dyn HistoryCell>,
    ) {
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
