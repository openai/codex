use super::super::TextArea;
use super::VimFind;
use super::VimFindKind;
use super::VimOperator;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use std::ops::Range;

impl VimFindKind {
    pub(in super::super) fn reverse(self) -> Self {
        match self {
            Self::FindForward => Self::FindBackward,
            Self::FindBackward => Self::FindForward,
            Self::TillForward => Self::TillBackward,
            Self::TillBackward => Self::TillForward,
        }
    }
}

impl TextArea {
    pub(in super::super) fn handle_vim_find_target(
        &mut self,
        operator: Option<VimOperator>,
        kind: VimFindKind,
        event: KeyEvent,
    ) -> bool {
        if self.vim_normal_keymap.cancel_operator.is_pressed(event)
            || self.vim_operator_keymap.cancel.is_pressed(event)
        {
            return true;
        }
        let KeyCode::Char(target) = event.code else {
            return true;
        };
        let find = VimFind { kind, target };
        self.vim_last_find = Some(find);
        self.execute_vim_find(operator, find);
        true
    }

    pub(in super::super) fn vim_normal_find_kind_for_event(
        &self,
        event: KeyEvent,
    ) -> Option<VimFindKind> {
        if self.vim_normal_keymap.find_forward.is_pressed(event) {
            return Some(VimFindKind::FindForward);
        }
        if self.vim_normal_keymap.find_backward.is_pressed(event) {
            return Some(VimFindKind::FindBackward);
        }
        if self.vim_normal_keymap.till_forward.is_pressed(event) {
            return Some(VimFindKind::TillForward);
        }
        if self.vim_normal_keymap.till_backward.is_pressed(event) {
            return Some(VimFindKind::TillBackward);
        }
        None
    }

    pub(in super::super) fn vim_operator_find_kind_for_event(
        &self,
        event: KeyEvent,
    ) -> Option<VimFindKind> {
        if self.vim_operator_keymap.find_forward.is_pressed(event) {
            return Some(VimFindKind::FindForward);
        }
        if self.vim_operator_keymap.find_backward.is_pressed(event) {
            return Some(VimFindKind::FindBackward);
        }
        if self.vim_operator_keymap.till_forward.is_pressed(event) {
            return Some(VimFindKind::TillForward);
        }
        if self.vim_operator_keymap.till_backward.is_pressed(event) {
            return Some(VimFindKind::TillBackward);
        }
        None
    }

    pub(in super::super) fn repeat_vim_find(
        &mut self,
        operator: Option<VimOperator>,
        reverse: bool,
    ) {
        let Some(mut find) = self.vim_last_find else {
            return;
        };
        if reverse {
            find.kind = find.kind.reverse();
        }
        self.execute_vim_find(operator, find);
    }

    fn execute_vim_find(&mut self, operator: Option<VimOperator>, find: VimFind) {
        if let Some(operator) = operator {
            if let Some(range) = self.range_for_find(find) {
                self.apply_vim_operator_to_range(operator, range);
            }
        } else if let Some(target) = self.target_for_find(find) {
            self.set_cursor(target);
        }
    }

    pub(in super::super) fn target_for_find(&self, find: VimFind) -> Option<usize> {
        let matched = self.find_match(find)?;
        Some(match find.kind {
            VimFindKind::FindForward | VimFindKind::FindBackward => matched,
            VimFindKind::TillForward => self.prev_atomic_boundary(matched),
            VimFindKind::TillBackward => self.next_atomic_boundary(matched),
        })
    }

    pub(in super::super) fn range_for_find(&self, find: VimFind) -> Option<Range<usize>> {
        let matched = self.find_match(find)?;
        let range = match find.kind {
            VimFindKind::FindForward => self.cursor_pos..self.next_atomic_boundary(matched),
            VimFindKind::TillForward => self.cursor_pos..matched,
            VimFindKind::FindBackward => matched..self.cursor_pos,
            VimFindKind::TillBackward => self.next_atomic_boundary(matched)..self.cursor_pos,
        };
        (range.start < range.end).then_some(range)
    }

    fn find_match(&self, find: VimFind) -> Option<usize> {
        let line_start = self.beginning_of_current_line();
        let line_end = self.end_of_current_line();
        match find.kind {
            VimFindKind::FindForward | VimFindKind::TillForward => {
                let start = self.next_atomic_boundary(self.cursor_pos).min(line_end);
                self.text[start..line_end]
                    .char_indices()
                    .map(|(offset, _)| start + offset)
                    .find(|&idx| self.matches_find_target(idx, find.target))
            }
            VimFindKind::FindBackward | VimFindKind::TillBackward => self.text
                [line_start..self.cursor_pos]
                .char_indices()
                .map(|(offset, _)| line_start + offset)
                .rev()
                .find(|&idx| self.matches_find_target(idx, find.target)),
        }
    }

    fn matches_find_target(&self, idx: usize, target: char) -> bool {
        !self.is_inside_element(idx)
            && self.clamp_pos_to_nearest_boundary(idx) == idx
            && self.text[idx..].starts_with(target)
    }
}
