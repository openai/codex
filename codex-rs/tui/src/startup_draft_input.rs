//! Edit and confirm the provisional startup draft without executing actions before initialization.
//!
//! Terminal input stays with the original pump through startup. Paste-newline ambiguity is
//! preserved, protected screens do not consume draft input, and confirmation never removes text.

use std::io;
use tokio::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

use super::STARTUP_PASTE_NEWLINE_TIMEOUT;
use super::StartupCancelled;
use super::StartupDraftInitialScreen;
use super::StartupDraftPump;
use crate::bottom_pane::BottomPane;
use crate::key_hint;
use crate::tui::Tui;
use crate::tui::TuiEvent;

impl StartupDraftPump {
    pub(super) fn handle_event(&mut self, tui: &mut Tui, event: TuiEvent) -> io::Result<()> {
        let screen_size = tui.screen_size_for_event(&event)?;
        if let Some((started_at, mut newlines)) = self.pending_paste_newline.take() {
            let continues_paste = match &event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Char(_),
                    modifiers,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => !key_hint::has_ctrl_or_alt(*modifiers),
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if modifiers.is_empty() => {
                    if started_at.elapsed() <= STARTUP_PASTE_NEWLINE_TIMEOUT {
                        newlines.push('\n');
                        self.pending_paste_newline = Some((Instant::now(), newlines));
                        return Ok(());
                    }
                    false
                }
                TuiEvent::Paste(text) => !text.is_empty(),
                TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::Resume | TuiEvent::FocusGained => {
                    self.pending_paste_newline = Some((started_at, newlines));
                    if self.initial_screen == StartupDraftInitialScreen::Composer {
                        self.draw(tui, screen_size)?;
                    }
                    return Ok(());
                }
                TuiEvent::FocusLost | TuiEvent::Mouse(_) => {
                    self.pending_paste_newline = Some((started_at, newlines));
                    return Ok(());
                }
                TuiEvent::Key(_) => false,
            };
            if continues_paste && started_at.elapsed() <= STARTUP_PASTE_NEWLINE_TIMEOUT {
                self.bottom_pane.handle_paste(newlines);
            }
        }
        match event {
            TuiEvent::Key(key) => {
                if self.initial_screen != StartupDraftInitialScreen::Composer
                    && !key_hint::ctrl(KeyCode::Char('c')).is_press(key)
                    && !key_hint::ctrl(KeyCode::Char('d')).is_press(key)
                {
                    return Ok(());
                }
                let key = match self.key_chord_matcher.advance(
                    key,
                    &self.key_chords,
                    self.bottom_pane.keymap_contexts(),
                ) {
                    crate::keymap::KeyChordMatch::Completed(key) => key,
                    crate::keymap::KeyChordMatch::PassThrough => key,
                    crate::keymap::KeyChordMatch::Pending(_)
                    | crate::keymap::KeyChordMatch::Cancelled
                    | crate::keymap::KeyChordMatch::Ignored => return Ok(()),
                };
                if key.code == KeyCode::Enter
                    && key.modifiers.is_empty()
                    && self.bottom_pane.is_in_paste_burst()
                {
                    let text_len = self.bottom_pane.composer_text().len();
                    self.bottom_pane.flush_composer_paste_burst();
                    if self
                        .bottom_pane
                        .composer_text()
                        .len()
                        .saturating_sub(text_len)
                        > 1
                        && (self.bottom_pane.is_startup_composer_action(key)
                            || !self.bottom_pane.is_safe_startup_editor_key(key))
                    {
                        self.pending_paste_newline = Some((Instant::now(), "\n".to_string()));
                    }
                }
                if self.initial_screen == StartupDraftInitialScreen::Composer
                    && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && self.session_action != super::StartupDraftSessionAction::NewFromCommandCenter
                    && self.bottom_pane.is_startup_submit_key(key)
                {
                    self.bottom_pane.flush_composer_paste_burst();
                    if self.pending_paste_newline.is_none()
                        && !self.bottom_pane.composer_text().trim().is_empty()
                    {
                        self.submission_pending = true;
                        self.bottom_pane.set_footer_hint_override(Some(vec![
                            ("Waiting for startup".into(), String::new()),
                            ("esc".into(), "cancel".into()),
                        ]));
                    }
                    crate::startup_recovery::remember(
                        self.bottom_pane.composer_recovery_snapshot(),
                    );
                    self.draw(tui, screen_size)?;
                    return Ok(());
                }
                if self.submission_pending
                    && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                    && key.code == KeyCode::Esc
                {
                    self.cancel_submission();
                    self.draw(tui, screen_size)?;
                    return Ok(());
                }
                let previous_text = self.bottom_pane.composer_text();
                if let Err(error) = handle_startup_draft_key(&mut self.bottom_pane, key)
                    && !(self.session_action
                        == super::StartupDraftSessionAction::NewFromCommandCenter
                        && StartupCancelled::matches(&error))
                {
                    if StartupCancelled::matches(&error)
                        && let Err(clear_error) = tui.terminal.clear()
                    {
                        tracing::warn!(
                            error = %clear_error,
                            "failed to clear the cancelled startup composer"
                        );
                    }
                    return Err(error);
                }
                if self.bottom_pane.composer_text() != previous_text
                    || self.bottom_pane.is_in_paste_burst()
                {
                    self.cancel_submission();
                }
            }
            TuiEvent::Paste(text) => {
                self.key_chord_matcher.cancel();
                if self.initial_screen == StartupDraftInitialScreen::Composer {
                    self.cancel_submission();
                    self.bottom_pane.flush_composer_paste_burst();
                    self.bottom_pane.handle_paste(text);
                }
            }
            TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::Resume | TuiEvent::FocusGained => {}
            TuiEvent::FocusLost => {
                self.key_chord_matcher.cancel();
                return Ok(());
            }
            TuiEvent::Mouse(_) => return Ok(()),
        }
        crate::startup_recovery::remember(self.bottom_pane.composer_recovery_snapshot());
        if self.initial_screen == StartupDraftInitialScreen::Composer {
            self.draw(tui, screen_size)?;
        }
        while self.app_event_rx.try_recv().is_ok() {}
        Ok(())
    }
}

pub(super) fn handle_startup_draft_key(
    bottom_pane: &mut BottomPane,
    key: KeyEvent,
) -> io::Result<()> {
    let _ = bottom_pane.flush_paste_burst_if_due();
    if key.kind == KeyEventKind::Release
        || key.code == KeyCode::Enter
            && key.modifiers.is_empty()
            && !bottom_pane.is_safe_startup_editor_key(key)
        || matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        || bottom_pane.is_startup_composer_action(key)
    {
        return Ok(());
    }

    if let KeyCode::Char(_) = key.code {
        let (code, modifiers) = key_hint::normalize_key_parts(key.code, key.modifiers);
        if key_hint::has_ctrl_or_alt(modifiers) && matches!(code, KeyCode::Char('r' | 'v')) {
            return Ok(());
        }
        let is_ctrl_c = key_hint::ctrl(KeyCode::Char('c')).is_press(key);
        let is_ctrl_d = key_hint::ctrl(KeyCode::Char('d')).is_press(key);
        if key.kind == KeyEventKind::Press && (is_ctrl_c || is_ctrl_d) {
            bottom_pane.flush_composer_paste_burst();
            if bottom_pane.composer_is_empty() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, StartupCancelled));
            }
            if is_ctrl_c {
                bottom_pane.on_ctrl_c();
                return Ok(());
            }
        }
    }

    if key_hint::has_ctrl_or_alt(key.modifiers) && !bottom_pane.is_safe_startup_editor_key(key)
        || key.code == KeyCode::Enter && !bottom_pane.is_safe_startup_editor_key(key)
        || key
            .modifiers
            .intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META)
        || !bottom_pane.is_safe_startup_editor_key(key)
            && !matches!(
                key.code,
                KeyCode::Char(_)
                    | KeyCode::Enter
                    | KeyCode::Esc
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::Backspace
                    | KeyCode::Delete
            )
    {
        return Ok(());
    }

    let _ = bottom_pane.handle_key_event(key);
    Ok(())
}
