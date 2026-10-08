//! Route registered read-only selections before copy, interrupt, clicks, and paste shortcuts.
//! A stationary click is replayed to the existing widget handler after selection releases it.

use super::*;
use crate::clipboard_copy::CopyFormat;
use crate::rendered_selection::RegionAction;
use crate::rendered_selection::RenderedSelection;
use crate::transcript_view::ViewAction;

impl App {
    pub(super) fn handle_rendered_selection_event(
        &mut self,
        tui: &mut tui::Tui,
        event: &TuiEvent,
    ) -> Result<bool> {
        if !tui.is_owned_screen()
            || self.overlay.is_some()
            || !self.chat_widget.no_modal_or_popup_active()
            || self.chat_widget.is_external_writer_view()
        {
            *self.chat_widget.rendered_selection.borrow_mut() = RenderedSelection::default();
            return Ok(false);
        }
        let refresh = matches!(event, TuiEvent::Mouse(mouse)
            if self.chat_widget.rendered_selection.borrow().needs_refresh(*mouse));
        if refresh {
            let size = tui.prepare_draw_size()?;
            self.render_owned_transcript(tui, size)?;
        }
        let action = {
            let mut selection = self.chat_widget.rendered_selection.borrow_mut();
            selection.view.copy_on_select = self
                .local_settings
                .copy_on_select(&codex_terminal_detection::terminal_info());
            selection.view.primary_selection = self.right_click_paste_environment.primary;
            selection.handle_event(event, tui.terminal.previous_buffer())
        };
        let Some(action) = action else {
            return Ok(false);
        };
        self.cancel_pending_key_chord();
        match action {
            RegionAction::Click(mouse) => {
                self.chat_widget
                    .handle_warning_event(&TuiEvent::Mouse(mouse), &self.transcript_cells);
            }
            RegionAction::Selection(action) => {
                self.transcript_view.end_selection(&self.transcript_cells);
                self.transcript_view.cancel_search();
                self.transcript_view.clear_activity_focus();
                self.chat_widget.clear_composer_selection();
                let copied = {
                    let mut selection = self.chat_widget.rendered_selection.borrow_mut();
                    let RenderedSelection { view, cells, .. } = &mut *selection;
                    view.copy_action(tui, cells, &action, CopyFormat::PlainText)
                };
                if let Some((characters, result)) = copied {
                    self.transcript_view.show_copy_feedback(&result, characters);
                } else if let ViewAction::OpenLink(url) = action {
                    self.open_url_in_browser(url);
                }
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(true)
    }
}
