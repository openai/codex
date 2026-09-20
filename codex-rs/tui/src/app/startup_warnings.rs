//! Coalesce retained startup diagnostics without adding live transcript rows or forcing reflow.

use super::*;
use crate::history_cell::StartupWarningsCell;

impl App {
    pub(super) fn merge_startup_warnings(
        &mut self,
        tui: &mut tui::Tui,
        incoming: &StartupWarningsCell,
    ) {
        let existing = self
            .transcript_cells
            .iter()
            .position(|cell| cell.as_any().is::<StartupWarningsCell>());
        let mut warnings = existing
            .map(|index| self.transcript_cells.remove(index))
            .and_then(|cell| cell.as_any().downcast_ref::<StartupWarningsCell>().cloned())
            .unwrap_or_default();
        for message in &incoming.messages {
            if !warnings.messages.contains(message) {
                warnings.messages.push(message.clone());
            }
        }
        warnings
            .other_sources
            .extend(incoming.other_sources.iter().cloned());
        warnings
            .mcp_servers
            .extend(incoming.mcp_servers.iter().cloned());
        for (server, messages) in &incoming.mcp_details {
            let details = warnings.mcp_details.entry(server.clone()).or_default();
            for message in messages {
                if !details.contains(message) {
                    details.push(message.clone());
                }
            }
        }
        // The final MCP summary must retain the individual diagnostics' sign-in subset.
        warnings
            .sign_in_servers
            .extend(incoming.sign_in_servers.iter().cloned());
        if warnings.messages.is_empty() {
            return;
        }
        let header = self
            .transcript_cells
            .iter()
            .rposition(|cell| cell.as_any().is::<history_cell::SessionInfoCell>());
        warnings.pending_header = header.is_none() && self.chat_widget.thread_id().is_none();
        self.transcript_cells.insert(
            header.map_or(/*default*/ 0, |index| index + 1),
            Arc::new(warnings),
        );
        self.native_history.retain(&self.transcript_cells);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.replace_cells(self.transcript_cells.clone());
        }
        if self.backtrack.overlay_preview_active {
            self.apply_backtrack_selection_internal(self.backtrack.nth_user_message);
        }
        tui.frame_requester().schedule_frame();
    }
}
