//! Input routing for detailed transcripts in owned and inline sessions.
//!
//! Preview selection is shared with the interactive transcript path so both
//! modes retain the same prompt navigation and confirmation behavior. Owned
//! preview entry respects active work and modal input owners.

use super::*;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::bindings_for_action;
use crate::keymap::keymap_action_ids;
use crossterm::event::KeyModifiers;

impl App {
    /// Reuse prompt selection without forwarding draws to the inline transcript overlay.
    pub(crate) fn handle_owned_backtrack_event(
        &mut self,
        tui: &mut tui::Tui,
        event: &TuiEvent,
    ) -> Result<bool> {
        if !tui.is_owned_screen() || self.overlay.is_some() {
            return Ok(false);
        }
        let TuiEvent::Key(key) = event else {
            return Ok(false);
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(false);
        }
        if self.backtrack.overlay_preview_active {
            if self.handle_backtrack_preview_key(tui, *key) {
                return Ok(true);
            }
            if self.keymap.pager.find.is_pressed(*key) {
                self.transcript_view.begin_search();
            } else if self.keymap.pager.close.is_pressed(*key)
                || self.keymap.pager.close_transcript.is_pressed(*key)
            {
                self.close_transcript_overlay(tui);
            } else if !self.transcript_view.navigate_pager(
                *key,
                &self.transcript_cells,
                &self.keymap.pager,
            ) {
                return Ok(false);
            }
            tui.frame_requester().schedule_frame();
            return Ok(true);
        }
        if !self.transcript_view.is_detailed() {
            return Ok(false);
        }
        if self.keymap.pager.close_transcript.is_pressed(*key) {
            if !self.transcript_view.has_active_interaction() {
                self.close_transcript_overlay(tui);
            }
            return Ok(true);
        }
        if key.code == KeyCode::Esc
            && key.kind == KeyEventKind::Press
            && self.should_handle_backtrack_esc(*key)
        {
            self.begin_overlay_backtrack_preview(tui);
            return Ok(true);
        }
        Ok(false)
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
        if interaction_active || (self.is_offline() && !self.backtrack.overlay_preview_active) {
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
        let browsing_needs_history = self.browsing_needs_history();
        let request = request.or_else(|| {
            let Some(Overlay::Transcript(overlay)) = &mut self.overlay else {
                return None;
            };
            let state = overlay.history_state();
            if (browsing_needs_history
                || overlay.needs_history()
                || state == TranscriptHistoryState::LoadingBeginning)
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
        overlay
            .should_load_older(*key)
            .then_some(TranscriptHistoryState::LoadingOlder)
    }

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

    pub(super) fn handle_backtrack_preview_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        if let TuiEvent::Key(key) = &event
            && self.handle_backtrack_preview_key(tui, *key)
        {
            return Ok(true);
        }
        self.overlay_forward_event(tui, event)?;
        Ok(true)
    }

    fn handle_backtrack_preview_key(&mut self, tui: &mut tui::Tui, key: KeyEvent) -> bool {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return false;
        }
        if self.keymap.app.open_transcript.is_pressed(key) {
            if key.kind == KeyEventKind::Press {
                self.set_browsing_details(!self.browsing_details());
                tui.frame_requester().schedule_frame();
            }
            return true;
        }
        if key.modifiers != KeyModifiers::NONE {
            return false;
        }
        if matches!(
            key.code,
            KeyCode::Up | KeyCode::Down | KeyCode::Char('h' | 'j' | 'k' | 'l')
        ) && keymap_action_ids()
            .filter(|action| action.context == KeymapContext::Pager)
            .any(|action| {
                bindings_for_action(&self.keymap, "pager", action.action)
                    .is_some_and(|bindings| bindings.is_pressed(key))
            })
        {
            return false;
        }
        match (key.code, key.kind) {
            (KeyCode::Enter, KeyEventKind::Press) if self.backtrack.base_id.is_some() => {
                if !self.is_offline() {
                    self.overlay_confirm_backtrack(tui);
                }
            }
            (KeyCode::Esc, KeyEventKind::Press) => {
                self.cancel_transcript_browsing(tui);
            }
            (
                code @ (KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l')),
                KeyEventKind::Press | KeyEventKind::Repeat,
            ) if self.backtrack.base_id.is_some() => {
                if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
                    overlay.cancel_pending_jump();
                } else {
                    self.transcript_view.cancel_beginning();
                }
                if matches!(code, KeyCode::Left | KeyCode::Char('h')) {
                    self.step_backtrack_and_highlight(tui);
                } else {
                    self.step_forward_backtrack_and_highlight(tui);
                }
            }
            (
                code @ (KeyCode::Up | KeyCode::Down | KeyCode::Char('j' | 'k')),
                KeyEventKind::Press | KeyEventKind::Repeat,
            ) => {
                let rows = if matches!(code, KeyCode::Up | KeyCode::Char('k')) {
                    -1
                } else {
                    1
                };
                if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
                    overlay.scroll(rows);
                } else {
                    self.transcript_view.scroll(&self.transcript_cells, rows);
                }
                tui.frame_requester().schedule_frame();
            }
            (KeyCode::Esc | KeyCode::Enter, KeyEventKind::Repeat) => {}
            _ => return false,
        }
        true
    }
}
