//! Manual recap progress presentation for `ChatWidget`.

use super::*;

impl ChatWidget {
    pub(crate) fn show_recap_loading(&mut self) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.transcript.active_cell = Some(Box::new(history_cell::ThreadRecapLoadingCell::new(
            self.local_settings.tui.animations && self.local_settings.tui.effects.progress,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    pub(crate) fn clear_recap_loading(&mut self) {
        let Some(active) = self.transcript.active_cell.as_ref() else {
            return;
        };
        if !active.as_any().is::<history_cell::ThreadRecapLoadingCell>() {
            return;
        }

        self.transcript.active_cell = None;
        self.bump_active_cell_revision();
        self.request_redraw();
    }
}
