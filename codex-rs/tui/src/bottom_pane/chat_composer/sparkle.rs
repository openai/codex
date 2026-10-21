//! One short Astra flourish for a confirmed new task with an untouched prompt.
//!
//! Ordinary draft content ends the opportunity permanently; slash commands can still select Astra
//! if the flourish has never started. The original field fills blank composer cells without drawing
//! over the placeholder or normal terminal cursor, and hidden frames do not schedule work. The
//! separate field renderer only paints its buffer and has no input or scheduling state. History
//! search hides an active field while its original deadline continues; its query and previews do
//! not count as input before acceptance. Shortcut help on an empty composer also hides an active
//! field without restarting its deadline and leaves an unused opportunity intact. The disconnected
//! editor shares draft tracking but treats composer-only shortcuts as ordinary editor input. A key
//! that finishes a paste burst reconciles the draft even if it removes a held slash in the same edit.

use std::cell::Cell;
use std::cell::RefCell;
use std::time::Duration;
use std::time::Instant;

use codex_config::types::Tui;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use super::ChatComposer;
use super::FooterMode;
use super::InputResult;
use crate::bottom_pane::BottomPane;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;

#[path = "sparkle_field.rs"]
mod field;

const FRAME_TICK: Duration = Duration::from_millis(/*millis*/ 150);
const IDLE_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 15);
const IDLE_FADE: Duration = Duration::from_secs(/*secs*/ 1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SparkleDraft {
    #[default]
    Untouched,
    Command,
    Dismissed,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Unarmed,
    Waiting,
    Visible(Instant),
    Finished,
}

pub(super) struct SparkleEditorState {
    text: String,
    paste_burst_active: bool,
}

pub(super) struct Sparkle {
    pub(super) draft: Cell<SparkleDraft>,
    /// Scoped to history's own replacements; other draft changes still count while search is open.
    pub(super) history_preview: bool,
    command_input: RefCell<String>,
    phase: Cell<Phase>,
    fresh: bool,
    terminal_focused: bool,
}

impl Default for Sparkle {
    fn default() -> Self {
        Self {
            draft: Cell::new(SparkleDraft::Untouched),
            history_preview: false,
            command_input: RefCell::new(String::new()),
            phase: Cell::new(Phase::Unarmed),
            fresh: false,
            terminal_focused: true,
        }
    }
}

impl BottomPane {
    pub(crate) fn is_sparkle_model(model: &str) -> bool {
        model
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .any(|part| part.eq_ignore_ascii_case("astra"))
    }

    pub(crate) fn mark_fresh_task_for_sparkle(&mut self, model: &str, settings: &Tui) {
        self.composer.sparkle.fresh = true;
        if matches!(
            self.composer.sparkle.draft.get(),
            SparkleDraft::Untouched | SparkleDraft::Command
        ) && self.composer.sparkle.phase.get() == Phase::Unarmed
            && settings.animations
            && settings.effects.starfield
            && Self::is_sparkle_model(model)
        {
            self.composer.sparkle.phase.set(Phase::Waiting);
            self.request_redraw();
        }
    }

    pub(crate) fn select_sparkle_model(&mut self, model: &str, settings: &Tui) {
        if self.composer.sparkle.fresh {
            self.mark_fresh_task_for_sparkle(model, settings);
        }
    }

    pub(crate) fn stop_ineligible_sparkle(&self, model: &str, settings: &Tui) {
        if !settings.animations || !settings.effects.starfield || !Self::is_sparkle_model(model) {
            self.composer.stop_visible_sparkle();
        }
    }

    pub(crate) fn dismiss_composer_sparkle(&self) {
        self.composer.dismiss_sparkle();
    }

    pub(crate) fn prepare_composer_sparkle_key(&self, key: KeyEvent) {
        if !self.has_active_view() {
            self.composer.prepare_sparkle_key(key);
        }
    }

    pub(crate) fn inherit_startup_sparkle(&self, draft: SparkleDraft) {
        if self.composer.sparkle.draft.get() == SparkleDraft::Untouched
            && draft != SparkleDraft::Untouched
        {
            self.composer.sparkle.draft.set(draft);
            if draft == SparkleDraft::Dismissed {
                self.composer.stop_visible_sparkle();
            }
        }
    }

    pub(crate) fn set_sparkle_terminal_focus(&mut self, focused: bool) {
        if self.composer.sparkle.terminal_focused != focused {
            self.composer.sparkle.terminal_focused = focused;
            self.request_redraw();
        }
    }
}

impl ChatComposer {
    fn prepare_sparkle_key(&self, key: KeyEvent) {
        if self.sparkle.draft.get() != SparkleDraft::Command
            && !(key.code == KeyCode::Char('/')
                && crate::key_hint::is_plain_text_key_event(key)
                && self.is_empty()
                && self.slash_commands_enabled())
        {
            self.stop_visible_sparkle();
        }
    }

    fn stop_visible_sparkle(&self) {
        if matches!(self.sparkle.phase.get(), Phase::Waiting | Phase::Visible(_)) {
            self.sparkle.phase.set(Phase::Finished);
            if let Some(requester) = &self.frame_requester {
                requester.schedule_frame();
            }
        }
    }

    pub(super) fn dismiss_sparkle(&self) {
        self.sparkle.draft.set(SparkleDraft::Dismissed);
        self.sparkle.command_input.borrow_mut().clear();
        self.stop_visible_sparkle();
    }

    pub(super) fn before_sparkle_key(&self, key: KeyEvent) -> Option<SparkleEditorState> {
        if self.history_search.is_some()
            || (Self::is_history_search_key(&key, &self.history_search_previous_keys)
                && (self.popups.active() || !self.draft.textarea.wants_vim_search_key(key))
                && !self.wants_vim_history_key(key))
            || self.empty_prompt_shortcut_toggle(&key).is_some()
        {
            return None;
        }
        self.before_sparkle_editor_key(key)
    }

    pub(super) fn before_sparkle_editor_key(&self, key: KeyEvent) -> Option<SparkleEditorState> {
        self.prepare_sparkle_key(key);
        if self.draft.textarea.vim_query().is_some() {
            return None;
        }
        if let KeyCode::Char(ch) = key.code
            && crate::key_hint::is_plain_text_key_event(key)
            && !key
                .modifiers
                .intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META)
            && !self.draft.textarea.is_vim_normal_mode()
        {
            match self.sparkle.draft.get() {
                SparkleDraft::Untouched
                    if ch == '/' && self.is_empty() && self.slash_commands_enabled() =>
                {
                    self.sparkle.draft.set(SparkleDraft::Command);
                    self.sparkle.command_input.replace(ch.to_string());
                }
                SparkleDraft::Command => {
                    let actual = self.current_text();
                    let mut projected = self.sparkle.command_input.borrow().clone();
                    let cursor = if actual == projected {
                        self.current_cursor()
                    } else {
                        projected.len()
                    };
                    projected.insert(cursor, ch);
                    if self.is_sparkle_command(&projected) {
                        self.sparkle.command_input.replace(projected);
                    } else {
                        self.dismiss_sparkle();
                    }
                }
                SparkleDraft::Untouched => self.dismiss_sparkle(),
                SparkleDraft::Dismissed => {}
            }
            None
        } else {
            Some(SparkleEditorState {
                text: self.current_text(),
                paste_burst_active: self.is_in_paste_burst(),
            })
        }
    }

    pub(super) fn after_sparkle_key(
        &self,
        before: Option<SparkleEditorState>,
        result: &InputResult,
    ) {
        match result {
            InputResult::Command(_) | InputResult::ServiceTierCommand(_) => {
                if self.sparkle.draft.get() == SparkleDraft::Command {
                    self.sparkle.draft.set(SparkleDraft::Untouched);
                    self.sparkle.command_input.borrow_mut().clear();
                }
            }
            InputResult::Submitted { .. }
            | InputResult::Queued { .. }
            | InputResult::ParentOwnedInputBlocked => self.dismiss_sparkle(),
            InputResult::CommandWithArgs(_, _, _) | InputResult::None => {
                if self.history_search.is_none() && self.draft.textarea.vim_query().is_none() {
                    let draft = self.current_text();
                    if before.is_some_and(|before| {
                        before.text != draft
                            || (before.paste_burst_active && !self.is_in_paste_burst())
                    }) {
                        self.note_sparkle_replaced_text(&draft);
                    }
                }
            }
        }
    }

    pub(super) fn note_sparkle_paste(&self, text: &str) {
        if self.history_search.is_some() {
            return;
        }
        if self.sparkle.draft.get() != SparkleDraft::Command
            && !(self.is_empty() && self.is_sparkle_command(text))
        {
            self.stop_visible_sparkle();
        }
        if !text.is_empty() && self.draft.textarea.vim_query().is_none() {
            match self.sparkle.draft.get() {
                SparkleDraft::Untouched if self.is_empty() && self.is_sparkle_command(text) => {
                    self.sparkle.draft.set(SparkleDraft::Command);
                    self.sparkle.command_input.replace(text.to_string());
                }
                SparkleDraft::Command => {
                    let mut projected = self.sparkle.command_input.borrow().clone();
                    let actual = self.current_text();
                    let cursor = if actual == projected {
                        self.current_cursor()
                    } else {
                        projected.len()
                    };
                    projected.insert_str(cursor, text);
                    if self.is_sparkle_command(&projected) {
                        self.sparkle.command_input.replace(projected);
                    } else {
                        self.dismiss_sparkle();
                    }
                }
                SparkleDraft::Untouched => self.dismiss_sparkle(),
                SparkleDraft::Dismissed => {}
            }
        }
    }

    pub(super) fn note_sparkle_replaced_text(&self, text: &str) {
        match self.sparkle.draft.get() {
            SparkleDraft::Command if text.is_empty() => {
                self.sparkle.draft.set(SparkleDraft::Untouched);
                self.sparkle.command_input.borrow_mut().clear();
            }
            SparkleDraft::Command if self.is_sparkle_command(text) => {
                self.sparkle.command_input.replace(text.to_string());
            }
            SparkleDraft::Untouched if text.is_empty() => {}
            SparkleDraft::Untouched | SparkleDraft::Command => self.dismiss_sparkle(),
            SparkleDraft::Dismissed => {}
        }
    }

    fn is_sparkle_command(&self, text: &str) -> bool {
        let Some(body) = text.strip_prefix('/') else {
            return false;
        };
        if !self.slash_commands_enabled() || self.draft.is_bash_mode || text.contains(['\n', '\r'])
        {
            return false;
        }
        let split = body.find(char::is_whitespace).unwrap_or(body.len());
        let (name, rest) = body.split_at(split);
        if rest.is_empty() {
            self.slash_input().is_editing_command_name(text, text.len())
                || self.slash_input().command(name).is_some()
        } else {
            self.slash_input()
                .command(name)
                .is_some_and(|command| rest.trim().is_empty() || command.supports_inline_args())
        }
    }

    pub(super) fn render_sparkle(
        &self,
        area: Rect,
        textarea: Rect,
        cursor: Option<(u16, u16)>,
        buf: &mut Buffer,
    ) {
        self.render_sparkle_at(area, textarea, cursor, Instant::now(), buf);
    }

    fn render_sparkle_at(
        &self,
        area: Rect,
        textarea: Rect,
        cursor: Option<(u16, u16)>,
        now: Instant,
        buf: &mut Buffer,
    ) {
        if self.history_search.is_none()
            && self.draft.textarea.vim_query().is_none()
            && !self.is_empty()
        {
            let is_command = self.sparkle.draft.get() == SparkleDraft::Command
                && self.is_sparkle_command(&self.current_text())
                && self.attachments.is_empty();
            if !is_command {
                self.dismiss_sparkle();
            }
        }
        let since = match self.sparkle.phase.get() {
            Phase::Visible(since) => Some(since),
            Phase::Waiting => None,
            Phase::Unarmed | Phase::Finished => return,
        };
        let elapsed = since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        if elapsed >= IDLE_TIMEOUT {
            self.stop_visible_sparkle();
            return;
        }
        if !self.has_focus
            || !self.sparkle.terminal_focused
            || !self.is_empty()
            || self.sparkle.draft.get() == SparkleDraft::Command
            || self.draft.paste_burst.is_active()
            || !self.draft.input_enabled
            || self.is_task_running
            || self.voice_strip.is_some()
            || self.popup_active()
            || self.footer_mode() == FooterMode::ShortcutOverlay
            || area.height < 3
            || textarea.is_empty()
            || effective_stdout_color_level() != StdoutColorLevel::TrueColor
        {
            return;
        }
        let Some(foreground) = default_fg() else {
            return;
        };
        if since.is_none() {
            self.sparkle.phase.set(Phase::Visible(now));
        }
        let fade_start = IDLE_TIMEOUT - IDLE_FADE;
        let visibility = if elapsed > fade_start {
            (IDLE_TIMEOUT - elapsed).as_secs_f32() / IDLE_FADE.as_secs_f32()
        } else {
            1.0
        };
        let protected = Rect::new(
            textarea.x,
            textarea.y,
            self.placeholder_text
                .width()
                .min(usize::from(textarea.width)) as u16,
            /*height*/ 1,
        );
        field::render_stars(
            area,
            cursor,
            Some(protected),
            elapsed.min(fade_start),
            foreground,
            visibility,
            buf,
        );
        if let Some(requester) = &self.frame_requester {
            requester.schedule_frame_in(FRAME_TICK.min(IDLE_TIMEOUT - elapsed));
        }
    }
}

#[cfg(test)]
#[path = "sparkle_tests.rs"]
mod tests;
