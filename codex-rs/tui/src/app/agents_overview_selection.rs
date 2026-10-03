//! Preserves Command Center selection across local and server-driven removals.
//! Choose against the displayed order before mutation; pending successors survive
//! batched removals and duplicate completion notifications until the next repaint.

use super::AGENTS_OVERVIEW_VIEW_ID;
use super::App;
use codex_protocol::ThreadId;
use std::collections::HashSet;

impl App {
    pub(in crate::app) fn prepare_agents_overview_removal(&mut self, removed: &HashSet<ThreadId>) {
        let selected = self.agents_overview.selection_after_removal.or_else(|| {
            let index = self
                .chat_widget
                .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)?;
            self.agents_overview.visible_thread_ids.get(index).copied()
        });
        let Some(selected) = selected.filter(|id| removed.contains(id)) else {
            return;
        };
        let threads = self
            .agents_overview
            .threads
            .values()
            .flatten()
            .cloned()
            .collect();
        self.agents_overview.selection_after_removal = self
            .agents_overview_view(threads, Some(selected))
            .selection_after_removal(removed);
    }
}
