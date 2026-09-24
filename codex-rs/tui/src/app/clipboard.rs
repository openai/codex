//! Dispatch clipboard key actions in order and apply completions to their selection owner.

use super::*;
use crate::chatwidget::KeyEventAction;
use crate::clipboard_copy::CopyFormat;

impl App {
    pub(super) fn handle_clipboard_key_action(
        &mut self,
        tui: &mut tui::Tui,
        action: KeyEventAction,
    ) {
        match action {
            KeyEventAction::None => {}
            KeyEventAction::CopyLastResponse(text) => {
                let result = tui
                    .clipboard
                    .copy(text, CopyFormat::Markdown, tui.frame_requester());
                self.chat_widget.show_copy_result("last message", result);
            }
            KeyEventAction::PasteImage => {
                // Finish paste before the next key, but never wait on the copy worker's lock.
                if tui.clipboard.is_busy() {
                    self.chat_widget.add_info_message(
                        "Copy already in progress; try image paste after it finishes".into(),
                        /*hint*/ None,
                    );
                } else {
                    self.chat_widget.paste_image();
                }
            }
        }
    }

    pub(super) fn finish_clipboard(&mut self, tui: &mut tui::Tui) {
        let current = tui.is_owned_screen() && self.overlay.is_none();
        let Some(completion) = tui.clipboard.poll() else {
            return;
        };
        if let Some(characters) = self.chat_widget.finish_clipboard(completion, current) {
            self.transcript_view
                .show_copy_feedback(&completion.1, characters);
        }
        let follow = self
            .transcript_view
            .finish_copy(&self.transcript_cells, completion, current);
        if follow == Some(true) {
            if self.backtrack.overlay_preview_active {
                self.close_transcript_overlay(tui);
            }
            self.transcript_view.jump_to_latest();
        }
    }
}
