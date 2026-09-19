//! Input routing for the read-only transcript viewer.
//!
//! Preview selection is shared with the interactive transcript path so both
//! modes retain the same prompt navigation and confirmation behavior.

use super::*;

impl App {
    pub(super) fn handle_legacy_transcript_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: TuiEvent,
    ) -> Result<bool> {
        if let TuiEvent::Key(key_event) = &event
            && let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut()
            && (overlay.should_load_older(*key_event)
                || (self.backtrack.overlay_preview_active
                    && self.backtrack.nth_user_message == 0
                    && matches!(key_event.code, KeyCode::Esc | KeyCode::Left)
                    && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)))
            && let Some(thread_id) = self.chat_widget.thread_id()
            && app_server.has_older_history(thread_id)
            && self.request_older_history_page(app_server, thread_id)
        {
            if let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut() {
                overlay.set_history_state(if overlay.should_load_from_start(*key_event) {
                    TranscriptHistoryState::LoadingBeginning
                } else {
                    TranscriptHistoryState::LoadingOlder
                });
            }
            tui.frame_requester().schedule_frame();
        }
        if self.backtrack.overlay_preview_active {
            return self.handle_backtrack_preview_event(tui, event);
        }
        match event {
            TuiEvent::Key(KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => self.begin_overlay_backtrack_preview(tui),
            event => self.overlay_forward_event(tui, event)?,
        }
        Ok(true)
    }

    pub(super) fn handle_backtrack_preview_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        match event {
            TuiEvent::Key(KeyEvent {
                code: KeyCode::Esc | KeyCode::Left,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => self.overlay_step_backtrack(tui, event)?,
            TuiEvent::Key(KeyEvent {
                code: KeyCode::Right,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => self.overlay_step_backtrack_forward(tui, event)?,
            TuiEvent::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => self.overlay_confirm_backtrack(tui),
            event => self.overlay_forward_event(tui, event)?,
        }
        Ok(true)
    }
}
