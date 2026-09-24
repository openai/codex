//! Handle clipboard feedback and image paste after the app checks worker availability.
//! Pending feedback belongs to this widget, so switching conversations discards it.

use super::*;
use crate::clipboard_copy::worker::CopyResult;

pub(super) enum PendingCopy {
    Message(u64, String),
    Selection(u64),
}

impl ChatWidget {
    pub(crate) fn paste_image(&mut self) {
        match paste_image_to_temp_png() {
            Ok((path, info)) => {
                tracing::debug!(
                    "pasted image size={}x{} format={}",
                    info.width,
                    info.height,
                    info.encoded_format.label()
                );
                self.attach_image(path);
            }
            Err(err) => {
                tracing::warn!("failed to paste image: {err}");
                self.add_to_history(history_cell::new_error_event(format!(
                    "Failed to paste image: {err}",
                )));
            }
        }
    }

    pub(super) fn show_clipboard_flash(&mut self, result: &CopyResult) {
        let line = match result {
            Ok(status) => Line::from(status.message("selection").dim()),
            Err(error) => Line::from(format!("Copy failed: {error}").red()),
        };
        self.bottom_pane
            .show_footer_flash(line, Duration::from_secs(/*secs*/ 3));
    }

    pub(crate) fn finish_clipboard(
        &mut self,
        completion: &(u64, CopyResult),
        composer_visible: bool,
    ) -> Option<usize> {
        let pending_id = match &self.pending_clipboard {
            Some(PendingCopy::Message(id, _) | PendingCopy::Selection(id)) => Some(*id),
            None => None,
        };
        if pending_id == Some(completion.0) {
            match self.pending_clipboard.take() {
                Some(PendingCopy::Message(_, label)) => match &completion.1 {
                    Ok(status) => self.add_info_message(status.message(&label), /*hint*/ None),
                    Err(error) => self.add_error_message(format!("Copy failed: {error}")),
                },
                Some(PendingCopy::Selection(_)) => self.show_clipboard_flash(&completion.1),
                None => {}
            }
        }
        self.bottom_pane
            .finish_composer_copy(completion, composer_visible)
    }
}
