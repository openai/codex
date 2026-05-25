use super::super::KillBufferKind;
use super::super::TextArea;
use super::VimInsertEdit;
use super::VimInsertRecording;
use super::VimMode;
use super::VimOperator;
use super::VimRepeatChange;
use super::VimRepeatTarget;
use super::VimVisualKind;
use crate::key_hint::KeyBindingListExt;
use crate::key_hint::is_altgr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;

impl TextArea {
    pub(in super::super) fn invalidate_vim_repeat_change(&mut self) {
        if !self.vim_replaying_change {
            self.vim_last_change = None;
            self.vim_insert_recording = None;
        }
    }

    pub(in super::super) fn record_vim_delete_if_changed(
        &mut self,
        target: VimRepeatTarget,
        previous_len: usize,
    ) {
        if !self.vim_replaying_change && self.text.len() < previous_len {
            self.vim_last_change = Some(VimRepeatChange::Delete(target));
            self.vim_insert_recording = None;
        }
    }

    pub(in super::super) fn begin_vim_change_recording(&mut self, target: VimRepeatTarget) {
        if !self.vim_replaying_change {
            self.vim_insert_recording = Some(VimInsertRecording {
                target,
                edits: Vec::new(),
            });
        }
    }

    pub(in super::super) fn complete_vim_change_recording(&mut self) {
        if self.vim_replaying_change {
            return;
        }
        if let Some(recording) = self.vim_insert_recording.take() {
            self.vim_last_change = Some(VimRepeatChange::Change {
                target: recording.target,
                edits: recording.edits,
            });
        }
    }

    pub(in super::super) fn record_vim_paste_if_changed(
        &mut self,
        text: String,
        kind: KillBufferKind,
        count: usize,
        previous_len: usize,
    ) {
        if !self.vim_replaying_change && self.text.len() > previous_len {
            self.vim_last_change = Some(VimRepeatChange::Paste { text, kind, count });
            self.vim_insert_recording = None;
        }
    }

    pub(in super::super) fn record_vim_insert_edit(&mut self, edit: Option<VimInsertEdit>) {
        if self.vim_replaying_change {
            return;
        }
        if let Some(recording) = self.vim_insert_recording.as_mut()
            && let Some(edit) = edit
        {
            recording.edits.push(edit);
        }
    }

    pub(in super::super) fn vim_insert_edit_for_event(
        &self,
        event: KeyEvent,
    ) -> Option<VimInsertEdit> {
        let keymap = &self.editor_keymap;
        if keymap.insert_newline.is_pressed(event) {
            return Some(VimInsertEdit::Insert("\n".to_string()));
        }
        if keymap.delete_backward_word.is_pressed(event) {
            return Some(VimInsertEdit::DeleteBackwardWord);
        }
        if let KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            ..
        } = event
            && is_altgr(modifiers)
        {
            return Some(VimInsertEdit::Insert(c.to_string()));
        }
        if keymap.delete_backward.is_pressed(event) {
            return Some(VimInsertEdit::DeleteBackward);
        }
        if keymap.delete_forward_word.is_pressed(event) {
            return Some(VimInsertEdit::DeleteForwardWord);
        }
        if keymap.delete_forward.is_pressed(event) {
            return Some(VimInsertEdit::DeleteForward);
        }
        if keymap.kill_line_start.is_pressed(event) {
            return Some(VimInsertEdit::KillLineStart);
        }
        if keymap.kill_whole_line.is_pressed(event) {
            return Some(VimInsertEdit::KillWholeLine);
        }
        if keymap.kill_line_end.is_pressed(event) {
            return Some(VimInsertEdit::KillLineEnd);
        }
        if keymap.yank.is_pressed(event) {
            return (!self.kill_buffer.is_empty())
                .then(|| VimInsertEdit::Insert(self.kill_buffer.clone()));
        }
        if keymap.move_word_left.is_pressed(event) {
            return Some(VimInsertEdit::MoveWordLeft);
        }
        if keymap.move_word_right.is_pressed(event) {
            return Some(VimInsertEdit::MoveWordRight);
        }
        if keymap.move_left.is_pressed(event) {
            return Some(VimInsertEdit::MoveLeft);
        }
        if keymap.move_right.is_pressed(event) {
            return Some(VimInsertEdit::MoveRight);
        }
        if keymap.move_up.is_pressed(event) {
            return Some(VimInsertEdit::MoveUp);
        }
        if keymap.move_down.is_pressed(event) {
            return Some(VimInsertEdit::MoveDown);
        }
        if keymap.move_line_start.is_pressed(event) {
            let move_up_at_bol = matches!(
                event,
                KeyEvent {
                    code: KeyCode::Char('a'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
            );
            return Some(VimInsertEdit::MoveLineStart { move_up_at_bol });
        }
        if keymap.move_line_end.is_pressed(event) {
            let move_down_at_eol = matches!(
                event,
                KeyEvent {
                    code: KeyCode::Char('e'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
            );
            return Some(VimInsertEdit::MoveLineEnd { move_down_at_eol });
        }
        if let KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            ..
        } = event
            && !c.is_ascii_control()
        {
            return Some(VimInsertEdit::Insert(c.to_string()));
        }
        None
    }

    pub(in super::super) fn repeat_vim_change(&mut self) {
        let Some(change) = self.vim_last_change.clone() else {
            return;
        };
        self.vim_replaying_change = true;
        match change {
            VimRepeatChange::Delete(target) => {
                self.execute_vim_repeat_target(VimOperator::Delete, target);
            }
            VimRepeatChange::Change { target, edits } => {
                if self.execute_vim_repeat_target(VimOperator::Change, target) {
                    for edit in edits {
                        self.apply_vim_insert_edit(edit);
                    }
                    self.exit_vim_insert_after_change();
                }
            }
            VimRepeatChange::Paste { text, kind, count } => {
                for _ in 0..count {
                    self.paste_text_after_cursor(&text, kind);
                }
            }
        }
        self.vim_replaying_change = false;
        self.clear_vim_counts();
        self.clear_vim_register_selection();
    }

    fn execute_vim_repeat_target(
        &mut self,
        operator: VimOperator,
        target: VimRepeatTarget,
    ) -> bool {
        let previous_len = self.text.len();
        match target {
            VimRepeatTarget::Characters(count) => {
                self.delete_forward_kill(count);
                if operator == VimOperator::Change {
                    self.vim_mode = VimMode::Insert;
                }
            }
            VimRepeatTarget::Lines(count) => match operator {
                VimOperator::Delete => self.vim_kill_current_lines(count),
                VimOperator::Change => self.vim_change_current_lines(count),
                VimOperator::Yank => unreachable!("repeat changes never yank"),
            },
            VimRepeatTarget::Motion { motion, count } => {
                self.apply_vim_operator(operator, motion, count);
            }
            VimRepeatTarget::TextObject {
                object,
                scope,
                count,
            } => {
                if let Some(range) = self.counted_text_object_range(object, scope, count) {
                    self.apply_vim_operator_to_range(operator, range, /*repeat_target*/ None);
                }
            }
            VimRepeatTarget::Find { find, count } => {
                if let Some(range) = self.range_for_find(find, count) {
                    self.apply_vim_operator_to_range(operator, range, /*repeat_target*/ None);
                }
            }
            VimRepeatTarget::Visual { kind, atomic_units } => {
                if let Some(mut range) = self.visual_repeat_range(kind, atomic_units) {
                    if operator == VimOperator::Change
                        && kind == VimVisualKind::Linewise
                        && range.end > range.start
                        && self.text[..range.end].ends_with('\n')
                    {
                        range.end -= 1;
                    }
                    let buffer_kind = if kind == VimVisualKind::Linewise {
                        KillBufferKind::Linewise
                    } else {
                        KillBufferKind::Characterwise
                    };
                    self.kill_range_with_kind(range, buffer_kind);
                    if operator == VimOperator::Change {
                        self.vim_mode = VimMode::Insert;
                    }
                }
            }
        }
        self.text.len() < previous_len
            || operator == VimOperator::Change && self.vim_mode == VimMode::Insert
    }

    pub(in super::super) fn visual_repeat_target(
        &self,
        kind: VimVisualKind,
        range: Range<usize>,
    ) -> VimRepeatTarget {
        let atomic_units = match kind {
            VimVisualKind::Characterwise => {
                let mut pos = range.start;
                let mut units = 0;
                while pos < range.end {
                    let next = self.next_atomic_boundary(pos);
                    if next <= pos {
                        break;
                    }
                    units += 1;
                    pos = next;
                }
                units.max(1)
            }
            VimVisualKind::Linewise => self.text[range].lines().count().max(1),
        };
        VimRepeatTarget::Visual { kind, atomic_units }
    }

    fn visual_repeat_range(
        &self,
        kind: VimVisualKind,
        atomic_units: usize,
    ) -> Option<Range<usize>> {
        let range = match kind {
            VimVisualKind::Characterwise => {
                let mut end = self.cursor_pos;
                for _ in 0..atomic_units {
                    end = self.next_atomic_boundary(end);
                }
                self.cursor_pos..end
            }
            VimVisualKind::Linewise => self.current_line_range_with_count(atomic_units),
        };
        (range.start < range.end).then(|| self.expand_range_to_element_boundaries(range))
    }

    fn apply_vim_insert_edit(&mut self, edit: VimInsertEdit) {
        match edit {
            VimInsertEdit::Insert(text) => self.insert_str(&text),
            VimInsertEdit::DeleteBackward => self.delete_backward(/*n*/ 1),
            VimInsertEdit::DeleteForward => self.delete_forward(/*n*/ 1),
            VimInsertEdit::DeleteBackwardWord => self.delete_backward_word(),
            VimInsertEdit::DeleteForwardWord => self.delete_forward_word(),
            VimInsertEdit::KillLineStart => self.kill_to_beginning_of_line(),
            VimInsertEdit::KillWholeLine => self.kill_current_line(),
            VimInsertEdit::KillLineEnd => self.kill_to_end_of_line(),
            VimInsertEdit::MoveLeft => self.move_cursor_left(),
            VimInsertEdit::MoveRight => self.move_cursor_right(),
            VimInsertEdit::MoveUp => self.move_cursor_up(),
            VimInsertEdit::MoveDown => self.move_cursor_down(),
            VimInsertEdit::MoveWordLeft => self.set_cursor(self.beginning_of_previous_word()),
            VimInsertEdit::MoveWordRight => self.set_cursor(self.end_of_next_word()),
            VimInsertEdit::MoveLineStart { move_up_at_bol } => {
                self.move_cursor_to_beginning_of_line(move_up_at_bol);
            }
            VimInsertEdit::MoveLineEnd { move_down_at_eol } => {
                self.move_cursor_to_end_of_line(move_down_at_eol);
            }
        }
    }

    pub(in super::super) fn exit_vim_insert_after_change(&mut self) {
        let bol = self.beginning_of_current_line();
        if self.cursor_pos > bol {
            self.cursor_pos = self.prev_atomic_boundary(self.cursor_pos).max(bol);
        }
        self.enter_vim_normal_mode();
    }
}
