use super::super::TextArea;
use super::VimMotion;
use super::VimOperator;
use super::VimTextObject;
use super::VimTextObjectScope;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;

impl TextArea {
    pub(in super::super) fn capture_vim_count_digit(&mut self, event: KeyEvent) -> bool {
        let KeyEvent {
            code: KeyCode::Char(digit @ '0'..='9'),
            modifiers: KeyModifiers::NONE,
            ..
        } = event
        else {
            return false;
        };
        if digit == '0' && self.vim_count.is_none() {
            return false;
        }
        let digit = digit.to_digit(10).unwrap_or_default() as usize;
        self.vim_count = Some(
            self.vim_count
                .unwrap_or_default()
                .saturating_mul(10)
                .saturating_add(digit),
        );
        true
    }

    pub(in super::super) fn begin_vim_operator(&mut self, operator: VimOperator) {
        self.vim_operator_count = self.vim_count.take();
        self.vim_pending = super::VimPending::Operator(operator);
    }

    pub(in super::super) fn take_vim_count(&mut self) -> usize {
        self.vim_count.take().unwrap_or(1).max(1)
    }

    pub(in super::super) fn take_vim_operator_count(&mut self) -> usize {
        let operator = self.vim_operator_count.take().unwrap_or(1);
        operator.saturating_mul(self.take_vim_count()).max(1)
    }

    pub(in super::super) fn clear_vim_counts(&mut self) {
        self.vim_count = None;
        self.vim_operator_count = None;
    }

    pub(in super::super) fn apply_counted_vim_normal_motion(
        &mut self,
        motion: VimMotion,
        count: usize,
    ) {
        self.clear_vim_register_selection();
        if motion == VimMotion::LineEnd {
            for _ in 1..count {
                self.move_cursor_down();
            }
            self.set_cursor(self.vim_line_end_cursor());
            return;
        }
        for _ in 0..count {
            match motion {
                VimMotion::Left => self.move_cursor_left(),
                VimMotion::Right => self.move_cursor_right(),
                VimMotion::Up => self.move_cursor_up(),
                VimMotion::Down => self.move_cursor_down(),
                VimMotion::WordForward => self.set_cursor(self.beginning_of_next_word()),
                VimMotion::WordBackward => self.set_cursor(self.beginning_of_previous_word()),
                VimMotion::WordEnd => self.set_cursor(self.vim_word_end_cursor()),
                VimMotion::LineStart => self.set_cursor(self.beginning_of_current_line()),
                VimMotion::LineEnd => unreachable!("handled before count loop"),
            }
        }
    }

    pub(in super::super) fn counted_text_object_range(
        &mut self,
        object: VimTextObject,
        scope: VimTextObjectScope,
        count: usize,
    ) -> Option<Range<usize>> {
        let original_cursor = self.cursor_pos;
        let mut aggregate = self.text_object_range(object, scope)?;
        for _ in 1..count {
            let previous_end = aggregate.end;
            let mut probe = previous_end;
            let mut next_range = None;
            while probe <= self.text.len() {
                self.cursor_pos = probe;
                if let Some(candidate) = self.text_object_range(object, scope)
                    && candidate.end > previous_end
                {
                    next_range = Some(candidate);
                    break;
                }
                if probe == self.text.len() {
                    break;
                }
                let next = self.next_atomic_boundary(probe);
                if next <= probe {
                    break;
                }
                probe = next;
            }
            let Some(next_range) = next_range else {
                break;
            };
            aggregate.start = aggregate.start.min(next_range.start);
            aggregate.end = aggregate.end.max(next_range.end);
        }
        self.cursor_pos = original_cursor;
        Some(aggregate)
    }

    pub(in super::super) fn vim_yank_current_lines(&mut self, count: usize) {
        let range = self.current_line_range_with_count(count);
        self.yank_line_range(range);
    }

    pub(in super::super) fn vim_kill_current_lines(&mut self, count: usize) {
        let range = self.current_line_range_with_count(count);
        self.kill_line_range(range);
    }

    pub(in super::super) fn vim_change_current_lines(&mut self, count: usize) {
        let range = self.current_lines_change_range(count);
        self.kill_line_range(range);
        self.vim_mode = super::VimMode::Insert;
    }

    pub(in super::super) fn current_line_range_with_count(&self, count: usize) -> Range<usize> {
        let start = self.beginning_of_current_line();
        let mut end = self.current_line_range_with_newline().end;
        for _ in 1..count {
            if end >= self.text.len() {
                break;
            }
            let eol = self.end_of_line(end);
            end = if eol < self.text.len() { eol + 1 } else { eol };
        }
        start..end
    }

    fn current_lines_change_range(&self, count: usize) -> Range<usize> {
        let start = self.beginning_of_current_line();
        let mut end = self.end_of_current_line();
        for _ in 1..count {
            if end >= self.text.len() {
                break;
            }
            end = self.end_of_line(end + 1);
        }
        start..end
    }
}
