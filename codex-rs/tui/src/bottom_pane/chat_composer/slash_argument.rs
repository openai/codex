//! Completion of finite first arguments for enabled slash commands.
//! Tab revalidates the live token before completing it; the configured submit shortcut
//! follows ordinary submission routing.

use std::ops::Range;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::ActivePopup;
use super::ChatComposer;
use super::InputResult;
use super::RestrictedInputMode;
use super::parent_owned_command_is_allowed;
use super::parse_slash_name;
use crate::bottom_pane::picker_style::selection_style;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::bottom_pane::slash_commands::SlashCommandItem;
use crate::bottom_pane::slash_commands::builtins_for_input;
use crate::key_hint::KeyBindingListExt;
use crate::slash_command::SlashCommand;

pub(super) fn choices(command: SlashCommand) -> &'static [&'static str] {
    match command {
        SlashCommand::Raw => &["on", "off"],
        SlashCommand::Ide => &["on", "off", "status"],
        SlashCommand::Usage => &["daily", "weekly", "cumulative"],
        SlashCommand::Voice => &["settings", "mute", "stop"],
        SlashCommand::Keymap => &["debug"],
        SlashCommand::Goal => &["clear", "edit", "pause", "resume"],
        SlashCommand::Mcp => &["verbose", "login"],
        SlashCommand::Pets => &["off"],
        _ => &[],
    }
}

struct Target {
    range: Range<usize>,
    prefix: String,
    identity: String,
    choices: Vec<&'static str>,
}

pub(super) struct ArgumentPopup {
    target: Target,
    matches: Vec<&'static str>,
    state: ScrollState,
}

impl ArgumentPopup {
    fn new(target: Target) -> Option<Self> {
        let prefix = target.prefix.to_ascii_lowercase();
        let matches: Vec<_> = target
            .choices
            .iter()
            .copied()
            .filter(|value| value.starts_with(&prefix))
            .collect();
        if matches.is_empty() {
            return None;
        }
        let mut state = ScrollState::new();
        state.clamp_selection(matches.len());
        Some(Self {
            target,
            matches,
            state,
        })
    }

    fn rows(&self) -> Vec<GenericDisplayRow> {
        self.matches
            .iter()
            .enumerate()
            .map(|(index, value)| GenericDisplayRow {
                name: (*value).to_owned(),
                name_prefix_spans: vec![if self.state.selected_idx == Some(index) {
                    "› ".into()
                } else {
                    "  ".into()
                }],
                selection_style: Some(selection_style()),
                match_indices: Some((0..self.target.prefix.chars().count()).collect()),
                ..Default::default()
            })
            .collect()
    }

    pub(super) fn calculate_required_height(&self, width: u16) -> u16 {
        measure_rows_height(&self.rows(), &self.state, MAX_POPUP_ROWS, width)
    }
}

impl WidgetRef for ArgumentPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        render_rows(
            area,
            buf,
            &self.rows(),
            &self.state,
            MAX_POPUP_ROWS,
            "no matches",
        );
    }
}

impl ChatComposer {
    fn slash_argument_target(&self) -> Option<Target> {
        let text = self.draft.textarea.text();
        let line = text.split('\n').next()?;
        let cursor = self.draft.textarea.cursor();
        if cursor > line.len() || !line.is_char_boundary(cursor) {
            return None;
        }
        let (name, _, start) = parse_slash_name(line)?;
        if cursor <= 1 + name.len() || cursor < start {
            return None;
        }
        let SlashCommandItem::Builtin(command) = self.slash_input().command(name)? else {
            return None;
        };
        let choices = choices(command);
        if choices.is_empty()
            || !builtins_for_input(self.builtin_command_flags())
                .iter()
                .any(|(_, visible)| *visible == command)
        {
            return None;
        }
        let end = start
            + line[start..]
                .find(char::is_whitespace)
                .unwrap_or(line.len() - start);
        if cursor > end
            || self
                .draft
                .textarea
                .text_element_ranges()
                .any(|r| r.start < end && r.end > start)
        {
            return None;
        }
        match self.popups.restricted_input_mode {
            Some(RestrictedInputMode::Disconnected | RestrictedInputMode::DraftHandoff) => {
                return None;
            }
            Some(RestrictedInputMode::UnavailableThread)
                if !command.available_when_thread_unavailable()
                    || text.trim().lines().count() != 1 =>
            {
                return None;
            }
            Some(RestrictedInputMode::UnavailableThread) | None => {}
        }
        let choices: Vec<_> = choices
            .iter()
            .copied()
            .filter(|value| {
                // Restricted recovery dispatch intentionally bypasses parent-owned input policy.
                if !self.blocks_direct_input
                    || self.popups.restricted_input_mode
                        == Some(RestrictedInputMode::UnavailableThread)
                {
                    return true;
                }
                let mut completed = text.to_owned();
                completed.replace_range(start..end, value);
                parse_slash_name(&completed)
                    .is_some_and(|(_, args, _)| parent_owned_command_is_allowed(command, args))
            })
            .collect();
        if choices.is_empty() {
            return None;
        }
        Some(Target {
            range: start..end,
            prefix: line[start..cursor].to_owned(),
            identity: line[..end].to_owned(),
            choices,
        })
    }

    pub(super) fn sync_slash_argument_popup(&mut self, allow: bool) -> bool {
        let target = allow.then(|| self.slash_argument_target()).flatten();
        if self
            .popups
            .dismissed_argument_token
            .as_ref()
            .is_some_and(|identity| target.as_ref().is_some_and(|t| &t.identity == identity))
        {
            return false;
        }
        self.popups.dismissed_argument_token = None;
        if let Some(target) = target {
            if let ActivePopup::Argument(popup) = &self.popups.active
                && popup.target.identity == target.identity
                && popup.target.prefix == target.prefix
                && popup.target.range == target.range
                && popup.target.choices == target.choices
            {
                return true;
            }
            if let Some(popup) = ArgumentPopup::new(target) {
                self.popups.active = ActivePopup::Argument(popup);
                return true;
            }
        }
        if matches!(self.popups.active, ActivePopup::Argument(_)) {
            self.popups.active = ActivePopup::None;
        }
        false
    }

    pub(super) fn handle_slash_argument_key(&mut self, key: KeyEvent) -> (InputResult, bool) {
        if self.submit_keys.is_pressed(key) {
            return self.handle_key_event_without_popup(key);
        }
        let ActivePopup::Argument(popup) = &mut self.popups.active else {
            unreachable!()
        };
        match (key.code, key.modifiers) {
            (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                popup.state.move_up_wrap(popup.matches.len());
                popup
                    .state
                    .ensure_visible(popup.matches.len(), MAX_POPUP_ROWS.min(popup.matches.len()));
            }
            (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                popup.state.move_down_wrap(popup.matches.len());
                popup
                    .state
                    .ensure_visible(popup.matches.len(), MAX_POPUP_ROWS.min(popup.matches.len()));
            }
            (KeyCode::Tab, _) => {
                if let Some(value) = popup
                    .state
                    .selected_idx
                    .and_then(|i| popup.matches.get(i))
                    .copied()
                {
                    let range = popup.target.range.clone();
                    let identity = popup.target.identity.clone();
                    let prefix = popup.target.prefix.clone();
                    if self.slash_argument_target().is_some_and(|target| {
                        target.range == range
                            && target.identity == identity
                            && target.prefix == prefix
                            && target.choices.contains(&value)
                    }) {
                        self.draft.textarea.replace_range(range.clone(), value);
                        self.draft.textarea.set_cursor(range.start + value.len());
                        self.popups.dismissed_argument_token =
                            self.slash_argument_target().map(|t| t.identity);
                    }
                }
                self.popups.active = ActivePopup::None;
            }
            (KeyCode::Esc, _) => {
                self.popups.dismissed_argument_token = Some(popup.target.identity.clone());
                self.popups.active = ActivePopup::None;
            }
            _ => return self.handle_key_event_without_popup(key),
        }
        (InputResult::None, true)
    }
}

#[cfg(test)]
#[path = "slash_argument_tests.rs"]
mod tests;
