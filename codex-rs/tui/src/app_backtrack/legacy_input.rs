//! Input routing for the read-only transcript viewer.
//!
//! Preview selection is shared with the interactive transcript path so both
//! modes retain the same prompt navigation and confirmation behavior.

use super::*;

impl App {
    fn request_legacy_transcript_history(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        request: Option<TranscriptHistoryState>,
    ) {
        if let Some(state) = request
            && let Some(thread_id) = self.chat_widget.thread_id()
            && app_server.has_older_history(thread_id)
            && self.request_older_history_page(app_server, thread_id)
            && let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut()
        {
            overlay.set_history_state(state);
            tui.frame_requester().schedule_frame();
        }
    }

    fn legacy_history_key_request(&mut self, event: &TuiEvent) -> Option<TranscriptHistoryState> {
        let (TuiEvent::Key(key), Some(Overlay::Transcript(overlay))) = (event, &mut self.overlay)
        else {
            return None;
        };
        if overlay.has_active_interaction() {
            return None;
        }
        if overlay.should_load_from_start(*key) {
            return Some(TranscriptHistoryState::LoadingBeginning);
        }
        let older_prompt = self.backtrack.overlay_preview_active
            && self.backtrack.nth_user_message == 0
            && matches!(key.code, KeyCode::Esc | KeyCode::Left)
            && key.modifiers.is_empty()
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat);
        (older_prompt || overlay.should_load_older(*key))
            .then_some(TranscriptHistoryState::LoadingOlder)
    }

    pub(super) fn handle_legacy_transcript_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: TuiEvent,
    ) -> Result<bool> {
        if let TuiEvent::Key(key) = &event
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && (key.code == KeyCode::Esc
                || (self.backtrack.overlay_preview_active
                    && matches!(key.code, KeyCode::Left | KeyCode::Right)))
            && let Some(Overlay::Transcript(overlay)) = self.overlay.as_mut()
        {
            overlay.cancel_pending_jump();
        }
        let request = self.legacy_history_key_request(&event);
        let explicit_input = matches!(event, TuiEvent::Key(_))
            || matches!(&event, TuiEvent::Mouse(mouse) if mouse.kind != crossterm::event::MouseEventKind::Moved);
        let interaction_active = matches!(&self.overlay, Some(Overlay::Transcript(overlay)) if overlay.has_active_interaction());
        if interaction_active || self.is_offline() {
            self.overlay_forward_event(tui, event)?;
        } else if self.backtrack.overlay_preview_active {
            self.handle_backtrack_preview_event(tui, event)?;
        } else {
            match event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                }) => self.begin_overlay_backtrack_preview(tui),
                event => self.overlay_forward_event(tui, event)?,
            }
        }
        let request = request.or_else(|| {
            let Some(Overlay::Transcript(overlay)) = &mut self.overlay else {
                return None;
            };
            let state = overlay.history_state();
            if (overlay.needs_history() || state == TranscriptHistoryState::LoadingBeginning)
                && (explicit_input || state != TranscriptHistoryState::Failed)
            {
                Some(if state == TranscriptHistoryState::LoadingBeginning {
                    state
                } else {
                    TranscriptHistoryState::LoadingOlder
                })
            } else {
                None
            }
        });
        self.request_legacy_transcript_history(tui, app_server, request);
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
