use super::super::KillBufferKind;
use super::super::TextArea;
use super::VimMode;
use super::VimMotion;
use super::VimOperator;
use super::VimPending;
use super::VimTextObjectScope;
use super::VimVisualKind;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;
use std::ops::Range;

impl TextArea {
    pub(in super::super) fn enter_vim_visual_mode(&mut self, kind: VimVisualKind) {
        self.vim_visual_anchor = Some(self.cursor_pos);
        self.vim_mode = VimMode::Visual(kind);
        self.vim_pending = VimPending::None;
        self.clear_vim_counts();
        self.clear_vim_register_selection();
    }

    pub(in super::super) fn clear_vim_visual_selection(&mut self) {
        self.vim_visual_anchor = None;
    }

    pub(crate) fn vim_visual_selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.vim_visual_anchor?;
        let VimMode::Visual(kind) = self.vim_mode else {
            return None;
        };
        if self.text.is_empty() {
            return None;
        }
        let range = match kind {
            VimVisualKind::Characterwise => {
                let start = anchor.min(self.cursor_pos);
                let end = self.next_atomic_boundary(anchor.max(self.cursor_pos));
                start..end
            }
            VimVisualKind::Linewise => {
                let start = self.beginning_of_line(anchor.min(self.cursor_pos));
                let line_end = self.end_of_line(anchor.max(self.cursor_pos));
                let end = if line_end < self.text.len() {
                    line_end + 1
                } else {
                    line_end
                };
                start..end
            }
        };
        (range.start < range.end).then(|| self.expand_range_to_element_boundaries(range))
    }

    pub(in super::super) fn handle_vim_visual(&mut self, event: KeyEvent) {
        if matches!(self.vim_pending, VimPending::None) && self.capture_vim_count_digit(event) {
            return;
        }
        let pending = std::mem::replace(&mut self.vim_pending, VimPending::None);
        match pending {
            VimPending::None => {}
            VimPending::Register => {
                self.handle_vim_register_name(event);
                return;
            }
            VimPending::Find {
                operator: None,
                kind,
            } => {
                self.handle_vim_find_target(/*operator*/ None, kind, event);
                return;
            }
            VimPending::VisualTextObject { scope } => {
                self.handle_vim_visual_text_object(scope, event);
                return;
            }
            VimPending::Operator(_)
            | VimPending::TextObject { .. }
            | VimPending::Find {
                operator: Some(_), ..
            } => {
                self.exit_vim_visual_mode();
                return;
            }
        }

        if self.vim_visual_keymap.cancel.is_pressed(event) {
            self.exit_vim_visual_mode();
            return;
        }
        if self.vim_normal_keymap.select_register.is_pressed(event) {
            self.vim_pending = VimPending::Register;
            return;
        }
        if self.vim_normal_keymap.enter_visual.is_pressed(event) {
            if self.vim_mode == VimMode::Visual(VimVisualKind::Characterwise) {
                self.exit_vim_visual_mode();
            } else {
                self.enter_vim_visual_mode(VimVisualKind::Characterwise);
            }
            return;
        }
        if self.vim_normal_keymap.enter_visual_line.is_pressed(event) {
            if self.vim_mode == VimMode::Visual(VimVisualKind::Linewise) {
                self.exit_vim_visual_mode();
            } else {
                self.enter_vim_visual_mode(VimVisualKind::Linewise);
            }
            return;
        }
        if self.vim_visual_keymap.delete.is_pressed(event) {
            self.apply_vim_visual_operator(VimOperator::Delete);
            return;
        }
        if self.vim_visual_keymap.yank.is_pressed(event) {
            self.apply_vim_visual_operator(VimOperator::Yank);
            return;
        }
        if self.vim_visual_keymap.change.is_pressed(event) {
            self.apply_vim_visual_operator(VimOperator::Change);
            return;
        }
        if let Some(scope) = self.vim_text_object_scope_for_event(event) {
            self.vim_pending = VimPending::VisualTextObject { scope };
            return;
        }
        if let Some(kind) = self.vim_normal_find_kind_for_event(event) {
            self.vim_pending = VimPending::Find {
                operator: None,
                kind,
            };
            return;
        }
        if self.vim_normal_keymap.repeat_find.is_pressed(event) {
            let count = self.take_vim_count();
            self.repeat_vim_find(/*operator*/ None, /*reverse*/ false, count);
            return;
        }
        if self.vim_normal_keymap.repeat_find_reverse.is_pressed(event) {
            let count = self.take_vim_count();
            self.repeat_vim_find(/*operator*/ None, /*reverse*/ true, count);
            return;
        }
        if let Some(motion) = self.vim_visual_motion_for_event(event) {
            let count = self.take_vim_count();
            self.apply_counted_vim_normal_motion(motion, count);
            return;
        }
        self.clear_vim_counts();
        self.clear_vim_register_selection();
    }

    fn vim_visual_motion_for_event(&self, event: KeyEvent) -> Option<VimMotion> {
        [
            (self.vim_normal_keymap.move_left.as_slice(), VimMotion::Left),
            (
                self.vim_normal_keymap.move_right.as_slice(),
                VimMotion::Right,
            ),
            (self.vim_normal_keymap.move_up.as_slice(), VimMotion::Up),
            (self.vim_normal_keymap.move_down.as_slice(), VimMotion::Down),
            (
                self.vim_normal_keymap.move_word_forward.as_slice(),
                VimMotion::WordForward,
            ),
            (
                self.vim_normal_keymap.move_word_backward.as_slice(),
                VimMotion::WordBackward,
            ),
            (
                self.vim_normal_keymap.move_word_end.as_slice(),
                VimMotion::WordEnd,
            ),
            (
                self.vim_normal_keymap.move_line_start.as_slice(),
                VimMotion::LineStart,
            ),
            (
                self.vim_normal_keymap.move_line_end.as_slice(),
                VimMotion::LineEnd,
            ),
        ]
        .into_iter()
        .find_map(|(bindings, motion)| bindings.is_pressed(event).then_some(motion))
    }

    fn handle_vim_visual_text_object(&mut self, scope: VimTextObjectScope, event: KeyEvent) {
        if self.vim_text_object_keymap.cancel.is_pressed(event) {
            self.clear_vim_counts();
            return;
        }
        let Some(object) = self.vim_text_object_for_event(event) else {
            self.clear_vim_counts();
            return;
        };
        let count = self.take_vim_count();
        if let Some(range) = self.counted_text_object_range(object, scope, count) {
            self.select_vim_visual_range(range);
        }
    }

    fn select_vim_visual_range(&mut self, range: Range<usize>) {
        let range = self.expand_range_to_element_boundaries(range);
        if range.start >= range.end {
            return;
        }
        self.vim_visual_anchor = Some(range.start);
        self.set_cursor(self.prev_atomic_boundary(range.end));
    }

    fn apply_vim_visual_operator(&mut self, operator: VimOperator) {
        let Some(mut range) = self.vim_visual_selection_range() else {
            self.exit_vim_visual_mode();
            return;
        };
        let linewise = self.vim_mode == VimMode::Visual(VimVisualKind::Linewise);
        if operator == VimOperator::Change
            && linewise
            && range.end > range.start
            && self.text[..range.end].ends_with('\n')
        {
            range.end -= 1;
        }
        let kind = if linewise {
            KillBufferKind::Linewise
        } else {
            KillBufferKind::Characterwise
        };
        let start = range.start;
        match operator {
            VimOperator::Delete => self.kill_range_with_kind(range, kind),
            VimOperator::Yank => self.yank_range_with_kind(range, kind),
            VimOperator::Change => self.kill_range_with_kind(range, kind),
        }
        self.clear_vim_visual_selection();
        self.clear_vim_counts();
        self.clear_vim_register_selection();
        if operator == VimOperator::Change {
            self.vim_mode = VimMode::Insert;
        } else {
            self.vim_mode = VimMode::Normal;
            self.set_cursor(start.min(self.vim_normal_end_cursor()));
        }
    }

    fn exit_vim_visual_mode(&mut self) {
        self.vim_mode = VimMode::Normal;
        self.vim_pending = VimPending::None;
        self.clear_vim_visual_selection();
        self.clear_vim_counts();
        self.clear_vim_register_selection();
    }
}
