//! Semantic Vim editing transactions, character find/till motions, and complete-change replay.
//!
//! `f`/`F` land on a matching grapheme; `t`/`T` stop just before/after it. Forward operator
//! motions include the destination, while backward motions exclude the original cursor.
//! All four motions share these boundaries for navigation, `c`/`d`/`y`, and semantic `.` replay.

use super::TextArea;
use super::VimMode;
use super::VimMotion;
use super::VimOperator;
use super::VimPending;
use super::VimTextObject;
use super::VimTextObjectScope;
use super::vim::VimFindMotion;
use crate::key_hint::KeyBindingListExt;
use crate::vim_search::SearchQuery;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use unicode_segmentation::GraphemeCursor;
use unicode_segmentation::UnicodeSegmentation;

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

#[derive(Clone, Debug)]
pub(super) enum VimEditTarget {
    Character,
    Line,
    LineEnd,
    Motion(VimMotion),
    Search(SearchQuery),
    TextObject {
        scope: VimTextObjectScope,
        object: VimTextObject,
    },
    Find {
        motion: VimFindMotion,
        target: char,
    },
    BufferJump {
        last: bool,
    },
}

#[derive(Clone, Debug)]
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
    pub(super) pending_change: Vec<VimEdit>,
    last_change: Vec<VimEdit>,
    changed: bool,
    pub(super) replaying: bool,
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
        self.apply_vim_editor_action(action.clone());
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
        self.vim_commands.pending_change = vec![VimEdit::Editor(VimEditorEdit(action.clone()))];
        self.vim_commands.changed = false;
        if !self.apply_vim_editor_action(action.clone()) {
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
            VimEdit::Editor(VimEditorEdit(action)) => self.apply_vim_editor_action(action.clone()),
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
        let is_change = matches!(action, VimAction::Change(_));
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
                let operator = if !is_change {
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
                    VimEditTarget::Search(query) => {
                        if !self.apply_vim_search(&query, Some(operator)) {
                            return false;
                        }
                    }
                    VimEditTarget::TextObject { scope, object } => {
                        let Some(range) = self.text_object_range(object, scope) else {
                            return false;
                        };
                        self.apply_vim_operator_to_range(operator, range);
                    }
                    VimEditTarget::Find { motion, target } => {
                        if !self.find_vim_character(motion, Some(operator), target) {
                            return false;
                        }
                    }
                    VimEditTarget::BufferJump { last } => {
                        self.jump_to_vim_buffer_line(last, Some(operator));
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
        if self.vim_normal_keymap.find_forward.is_pressed(event) {
            self.start_vim_find(VimFindMotion::Forward, /*operator*/ None);
        } else if self.vim_normal_keymap.find_backward.is_pressed(event) {
            self.start_vim_find(VimFindMotion::Backward, /*operator*/ None);
        } else if self.vim_normal_keymap.till_forward.is_pressed(event) {
            self.start_vim_find(VimFindMotion::TillForward, /*operator*/ None);
        } else if self.vim_normal_keymap.till_backward.is_pressed(event) {
            self.start_vim_find(VimFindMotion::TillBackward, /*operator*/ None);
        } else if self.vim_normal_keymap.jump_top.is_pressed(event) {
            self.jump_to_vim_buffer_line(/*last*/ false, /*operator*/ None);
        } else if self.vim_normal_keymap.jump_bottom.is_pressed(event) {
            self.jump_to_vim_buffer_line(/*last*/ true, /*operator*/ None);
        } else {
            return false;
        }
        true
    }

    pub(super) fn handle_vim_operator_command(
        &mut self,
        operator: VimOperator,
        event: KeyEvent,
    ) -> bool {
        if self
            .vim_operator_keymap
            .motion_find_forward
            .is_pressed(event)
        {
            self.start_vim_find(VimFindMotion::Forward, Some(operator));
        } else if self
            .vim_operator_keymap
            .motion_find_backward
            .is_pressed(event)
        {
            self.start_vim_find(VimFindMotion::Backward, Some(operator));
        } else if self
            .vim_operator_keymap
            .motion_till_forward
            .is_pressed(event)
        {
            self.start_vim_find(VimFindMotion::TillForward, Some(operator));
        } else if self
            .vim_operator_keymap
            .motion_till_backward
            .is_pressed(event)
        {
            self.start_vim_find(VimFindMotion::TillBackward, Some(operator));
        } else if self.vim_operator_keymap.motion_jump_top.is_pressed(event)
            || self
                .vim_operator_keymap
                .motion_jump_bottom
                .is_pressed(event)
        {
            let last = self
                .vim_operator_keymap
                .motion_jump_bottom
                .is_pressed(event);
            match operator {
                VimOperator::Delete => {
                    self.start_vim_edit(VimAction::Delete(VimEditTarget::BufferJump { last }));
                }
                VimOperator::Change => {
                    self.start_vim_edit(VimAction::Change(VimEditTarget::BufferJump { last }));
                }
                VimOperator::Yank => self.jump_to_vim_buffer_line(last, Some(operator)),
            }
        } else {
            return false;
        }
        true
    }

    pub(super) fn handle_vim_pending_command(&mut self, pending: VimPending, event: KeyEvent) {
        match pending {
            VimPending::Replace => {
                if let Some(ch) = vim_command_char(event) {
                    self.start_vim_edit(VimAction::Replace(ch));
                }
            }
            VimPending::Find { motion, operator } => {
                if let Some(ch) = vim_command_char(event) {
                    match operator {
                        Some(VimOperator::Delete) => {
                            self.start_vim_edit(VimAction::Delete(VimEditTarget::Find {
                                motion,
                                target: ch,
                            }));
                        }
                        Some(VimOperator::Change) => {
                            self.start_vim_edit(VimAction::Change(VimEditTarget::Find {
                                motion,
                                target: ch,
                            }));
                        }
                        Some(VimOperator::Yank) | None => {
                            self.find_vim_character(motion, operator, ch);
                        }
                    }
                }
            }
            VimPending::None | VimPending::Operator(_) | VimPending::TextObject { .. } => {}
        }
    }

    fn start_vim_find(&mut self, motion: VimFindMotion, operator: Option<VimOperator>) {
        self.vim_pending = VimPending::Find { motion, operator };
    }

    fn find_vim_character(
        &mut self,
        motion: VimFindMotion,
        operator: Option<VimOperator>,
        target: char,
    ) -> bool {
        let origin = self.cursor_pos;
        let found = match motion {
            VimFindMotion::Forward | VimFindMotion::TillForward => {
                let line_end = self.end_of_current_line();
                if origin >= line_end {
                    return false;
                }
                let start = self.next_atomic_boundary(origin);
                self.text[start..line_end]
                    .grapheme_indices(/*is_extended*/ true)
                    .find(|(offset, grapheme)| {
                        grapheme.starts_with(target) && self.is_vim_command_target(start + offset)
                    })
                    .map(|(offset, grapheme)| start + offset..start + offset + grapheme.len())
            }
            VimFindMotion::Backward | VimFindMotion::TillBackward => {
                let line_start = self.beginning_of_current_line();
                self.text[line_start..origin]
                    .grapheme_indices(/*is_extended*/ true)
                    .rev()
                    .find(|(offset, grapheme)| {
                        grapheme.starts_with(target)
                            && self.is_vim_command_target(line_start + offset)
                    })
                    .map(|(offset, grapheme)| {
                        let start = line_start + offset;
                        start..start + grapheme.len()
                    })
            }
        };
        let Some(position) = found else {
            return false;
        };
        if let Some(operator) = operator {
            let range = match motion {
                VimFindMotion::Forward => origin..position.end,
                VimFindMotion::Backward => position.start..origin,
                VimFindMotion::TillForward => origin..position.start,
                VimFindMotion::TillBackward => position.end..origin,
            };
            if operator == VimOperator::Yank {
                self.set_cursor(range.start);
            }
            self.apply_vim_operator_to_range(operator, range);
        } else {
            let destination = match motion {
                VimFindMotion::Forward | VimFindMotion::Backward => position.start,
                VimFindMotion::TillForward => {
                    let previous = self.text[..position.start]
                        .grapheme_indices(/*is_extended*/ true)
                        .next_back()
                        .map_or(origin, |(offset, _)| offset);
                    self.find_element_containing(previous)
                        .map_or(previous, |idx| self.elements[idx].range.start)
                }
                VimFindMotion::TillBackward => position.end,
            };
            self.set_cursor(destination);
        }
        true
    }

    fn jump_to_vim_buffer_line(&mut self, last: bool, operator: Option<VimOperator>) {
        if let Some(operator) = operator {
            let current = self.current_line_range_with_newline();
            let range = if last {
                current.start..self.text.len()
            } else {
                0..current.end
            };
            match operator {
                VimOperator::Delete => self.kill_line_range(range),
                VimOperator::Yank => self.yank_line_range(range),
                VimOperator::Change => {
                    self.kill_line_range(range);
                    self.vim_mode = VimMode::Insert;
                }
            }
            return;
        }
        let start = if last {
            self.beginning_of_line(self.text.len())
        } else {
            0
        };
        self.set_cursor(start);
        self.set_cursor(self.first_non_blank_of_current_line());
    }

    pub(super) fn is_vim_command_target(&self, position: usize) -> bool {
        !self
            .elements
            .iter()
            .any(|element| element.range.contains(&position))
            && GraphemeCursor::new(position, self.text.len(), /*is_extended*/ true)
                .is_boundary(&self.text, /*chunk_start*/ 0)
                .unwrap_or(false)
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
