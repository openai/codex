//! Semantic Vim editing transactions and complete-change replay.

use super::TextArea;
use super::VimMode;
use super::VimMotion;
use super::VimOperator;
use super::VimPending;
use super::VimTextObject;
use super::VimTextObjectScope;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

#[derive(Clone, Debug)]
pub(crate) enum VimEdit {
    Editor(VimEditorEdit),
    Text(String),
}

#[derive(Clone, Debug)]
pub(crate) struct VimEditorEdit(VimAction);

#[derive(Clone, Copy, Debug)]
pub(super) enum VimInsertPosition {
    Cursor,
    AfterCursor,
    LineStart,
    LineEnd,
    OpenAbove,
    OpenBelow,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum VimEditTarget {
    Character,
    Line,
    LineEnd,
    Motion(VimMotion),
    TextObject {
        scope: VimTextObjectScope,
        object: VimTextObject,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum VimAction {
    Insert(VimInsertPosition),
    Delete(VimEditTarget),
    Change(VimEditTarget),
    Replace(char),
    PasteAfter,
    DeleteBackward,
    DeleteForward,
    DeleteBackwardWord,
    DeleteForwardWord,
    KillLineStart,
    KillLine,
    KillLineEnd,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    MoveLineStart { move_up_at_bol: bool },
    MoveLineEnd { move_down_at_eol: bool },
}

#[derive(Debug, Default)]
pub(super) struct VimCommandState {
    pending_change: Vec<VimEdit>,
    last_change: Vec<VimEdit>,
    changed: bool,
    replaying: bool,
}

impl TextArea {
    pub(crate) fn vim_repeat_actions(&self) -> Option<Vec<VimEdit>> {
        (!self.vim_commands.last_change.is_empty()).then(|| self.vim_commands.last_change.clone())
    }

    pub(super) fn record_vim_inserted_text(&mut self, text: &str) {
        if !self.vim_enabled
            || self.vim_mode != VimMode::Insert
            || self.vim_commands.replaying
            || self.vim_commands.pending_change.is_empty()
            || text.is_empty()
        {
            return;
        }
        if let Some(VimEdit::Text(pending)) = self.vim_commands.pending_change.last_mut() {
            pending.push_str(text);
        } else {
            self.vim_commands
                .pending_change
                .push(VimEdit::Text(text.to_owned()));
        }
        self.vim_commands.changed = true;
    }

    pub(super) fn apply_vim_insert_action(&mut self, action: VimAction) {
        let recording = self.vim_enabled
            && self.vim_mode == VimMode::Insert
            && !self.vim_commands.replaying
            && !self.vim_commands.pending_change.is_empty();
        let prior_len = self.text.len();
        self.apply_vim_editor_action(action);
        let changed = self.text.len() != prior_len;
        let deletion = matches!(
            action,
            VimAction::DeleteBackward
                | VimAction::DeleteForward
                | VimAction::DeleteBackwardWord
                | VimAction::DeleteForwardWord
                | VimAction::KillLineStart
                | VimAction::KillLine
                | VimAction::KillLineEnd
        );
        if recording && (changed || !deletion) {
            self.vim_commands
                .pending_change
                .push(VimEdit::Editor(VimEditorEdit(action)));
        }
        self.vim_commands.changed |= recording && changed;
    }

    pub(super) fn start_vim_edit(&mut self, action: VimAction) -> bool {
        let prior_len = self.text.len();
        self.vim_commands.pending_change = vec![VimEdit::Editor(VimEditorEdit(action))];
        self.vim_commands.changed = false;
        if !self.apply_vim_editor_action(action) {
            self.vim_commands.pending_change.clear();
            return false;
        }
        self.vim_commands.changed =
            self.text.len() != prior_len || matches!(action, VimAction::Replace(_));
        if self.vim_mode == VimMode::Normal {
            self.finish_pending_vim_change();
        }
        true
    }

    pub(super) fn finish_pending_vim_change(&mut self) {
        if self.vim_commands.changed {
            self.vim_commands.last_change = std::mem::take(&mut self.vim_commands.pending_change);
        } else {
            self.vim_commands.pending_change.clear();
        }
        self.vim_commands.changed = false;
    }

    pub(crate) fn begin_vim_repeat(&mut self) -> Option<Vec<VimEdit>> {
        let edits = self.vim_repeat_actions()?;
        self.vim_commands.replaying = true;
        Some(edits)
    }

    pub(crate) fn finish_vim_repeat(&mut self) {
        if self.vim_mode == VimMode::Insert {
            self.leave_vim_insert_mode();
        }
        self.vim_pending = VimPending::None;
        self.vim_commands.replaying = false;
    }

    pub(crate) fn apply_vim_edit(&mut self, edit: &VimEdit) -> bool {
        match edit {
            VimEdit::Editor(VimEditorEdit(action)) => self.apply_vim_editor_action(*action),
            VimEdit::Text(text) => {
                if self.vim_mode != VimMode::Insert {
                    return false;
                }
                self.insert_str(text);
                true
            }
        }
    }

    fn apply_vim_editor_action(&mut self, action: VimAction) -> bool {
        let prior_len = self.text.len();
        match action {
            VimAction::Insert(position) => {
                match position {
                    VimInsertPosition::Cursor => {}
                    VimInsertPosition::AfterCursor => {
                        self.set_cursor(self.next_atomic_boundary(self.cursor_pos));
                    }
                    VimInsertPosition::LineStart => {
                        self.set_cursor(self.first_non_blank_of_current_line());
                    }
                    VimInsertPosition::LineEnd => self.set_cursor(self.end_of_current_line()),
                    VimInsertPosition::OpenAbove => {
                        let bol = self.beginning_of_current_line();
                        self.insert_str_at(bol, "\n");
                        self.set_cursor(bol);
                    }
                    VimInsertPosition::OpenBelow => {
                        let eol = self.end_of_current_line();
                        let insert_at = if eol < prior_len { eol + 1 } else { eol };
                        self.insert_str_at(insert_at, "\n");
                        self.set_cursor(if eol < prior_len {
                            insert_at
                        } else {
                            insert_at + 1
                        });
                    }
                }
                self.vim_mode = VimMode::Insert;
            }
            VimAction::Delete(target) | VimAction::Change(target) => {
                let operator = if matches!(action, VimAction::Delete(_)) {
                    VimOperator::Delete
                } else {
                    VimOperator::Change
                };
                match target {
                    VimEditTarget::Character => {
                        if self.cursor_pos < self.end_of_current_line() {
                            self.delete_forward_kill(/*n*/ 1);
                        }
                        if operator == VimOperator::Change {
                            self.vim_mode = VimMode::Insert;
                        }
                    }
                    VimEditTarget::Line => {
                        if operator == VimOperator::Delete {
                            self.kill_current_line();
                        } else {
                            let range =
                                self.beginning_of_current_line()..self.end_of_current_line();
                            self.kill_line_range(range);
                            self.vim_mode = VimMode::Insert;
                        }
                    }
                    VimEditTarget::LineEnd => {
                        self.vim_kill_to_end_of_line();
                        if operator == VimOperator::Change {
                            self.vim_mode = VimMode::Insert;
                        }
                    }
                    VimEditTarget::Motion(motion) => self.apply_vim_operator(operator, motion),
                    VimEditTarget::TextObject { scope, object } => {
                        let Some(range) = self.text_object_range(object, scope) else {
                            return false;
                        };
                        self.apply_vim_operator_to_range(operator, range);
                    }
                }
                if operator == VimOperator::Change {
                    return self.vim_mode == VimMode::Insert;
                }
                return self.text.len() != prior_len;
            }
            VimAction::Replace(ch) => {
                if self.cursor_pos >= self.end_of_current_line() {
                    return false;
                }
                let start = self.cursor_pos;
                let end = self.next_atomic_boundary(start);
                self.replace_range(start..end, &ch.to_string());
                self.set_cursor(start + usize::from(ch == '\n'));
            }
            VimAction::PasteAfter => {
                self.paste_after_cursor();
                return self.text.len() != prior_len;
            }
            VimAction::DeleteBackward => self.delete_backward(/*n*/ 1),
            VimAction::DeleteForward => self.delete_forward(/*n*/ 1),
            VimAction::DeleteBackwardWord => self.delete_backward_word(),
            VimAction::DeleteForwardWord => self.delete_forward_word(),
            VimAction::KillLineStart => self.kill_to_beginning_of_line(),
            VimAction::KillLine => self.kill_current_line(),
            VimAction::KillLineEnd => self.kill_to_end_of_line(),
            VimAction::MoveLeft => self.move_cursor_left(),
            VimAction::MoveRight => self.move_cursor_right(),
            VimAction::MoveUp => self.move_cursor_up(),
            VimAction::MoveDown => self.move_cursor_down(),
            VimAction::MoveWordLeft => self.set_cursor(self.beginning_of_previous_word()),
            VimAction::MoveWordRight => self.set_cursor(self.end_of_next_word()),
            VimAction::MoveLineStart { move_up_at_bol } => {
                self.move_cursor_to_beginning_of_line(move_up_at_bol);
            }
            VimAction::MoveLineEnd { move_down_at_eol } => {
                self.move_cursor_to_end_of_line(move_down_at_eol);
            }
        }
        true
    }

    pub(super) fn handle_vim_extra_command(&mut self, event: KeyEvent) -> bool {
        if self.vim_normal_keymap.replace_char.is_pressed(event)
            && self.cursor_pos < self.end_of_current_line()
        {
            self.vim_pending = VimPending::Replace;
            return true;
        }
        if self.vim_normal_keymap.repeat_last_change.is_pressed(event) {
            if let Some(edits) = self.begin_vim_repeat() {
                for edit in edits {
                    if !self.apply_vim_edit(&edit) {
                        break;
                    }
                }
                self.finish_vim_repeat();
            }
            return true;
        }
        false
    }

    pub(super) fn handle_vim_pending_command(&mut self, pending: VimPending, event: KeyEvent) {
        match pending {
            VimPending::Replace => {
                if let Some(ch) = vim_command_char(event) {
                    self.start_vim_edit(VimAction::Replace(ch));
                }
            }
            VimPending::None | VimPending::Operator(_) | VimPending::TextObject { .. } => {}
        }
    }
}

fn vim_command_char(event: KeyEvent) -> Option<char> {
    if event.code == KeyCode::Enter {
        return Some('\n');
    }
    let KeyCode::Char(ch) = event.code else {
        return None;
    };
    match event.modifiers {
        KeyModifiers::NONE => Some(ch),
        KeyModifiers::SHIFT => Some(if ch.is_ascii_lowercase() {
            ch.to_ascii_uppercase()
        } else {
            ch
        }),
        modifiers if crate::key_hint::is_altgr(modifiers) => Some(ch),
        _ => None,
    }
}

#[cfg(test)]
#[path = "vim_commands_tests.rs"]
mod tests;
