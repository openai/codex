use super::TextArea;
use super::split_word_pieces;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimMode {
    /// Normal mode routes printable keys to movement, operators, and mode transitions.
    Normal,
    /// Insert mode routes input through the regular editor keymap until Escape is pressed.
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimOperator {
    Delete,
    Yank,
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimPending {
    None,
    Operator(VimOperator),
    TextObject {
        operator: VimOperator,
        scope: VimTextObjectScope,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimMotion {
    Left,
    Right,
    Up,
    Down,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    LineEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimTextObjectScope {
    Inner,
    Around,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VimTextObject {
    Word,
    BigWord,
    Parentheses,
    Brackets,
    Braces,
    DoubleQuote,
    SingleQuote,
    Backtick,
}

impl TextArea {
    pub(super) fn vim_text_object_scope_for_event(
        &self,
        event: KeyEvent,
    ) -> Option<VimTextObjectScope> {
        if self
            .vim_operator_keymap
            .select_inner_text_object
            .is_pressed(event)
        {
            return Some(VimTextObjectScope::Inner);
        }
        if self
            .vim_operator_keymap
            .select_around_text_object
            .is_pressed(event)
        {
            return Some(VimTextObjectScope::Around);
        }
        None
    }

    pub(super) fn vim_text_object_for_event(&self, event: KeyEvent) -> Option<VimTextObject> {
        if self.vim_text_object_keymap.word.is_pressed(event) {
            return Some(VimTextObject::Word);
        }
        if self.vim_text_object_keymap.big_word.is_pressed(event) {
            return Some(VimTextObject::BigWord);
        }
        if self.vim_text_object_keymap.parentheses.is_pressed(event) {
            return Some(VimTextObject::Parentheses);
        }
        if self.vim_text_object_keymap.brackets.is_pressed(event) {
            return Some(VimTextObject::Brackets);
        }
        if self.vim_text_object_keymap.braces.is_pressed(event) {
            return Some(VimTextObject::Braces);
        }
        if self.vim_text_object_keymap.double_quote.is_pressed(event) {
            return Some(VimTextObject::DoubleQuote);
        }
        if self.vim_text_object_keymap.single_quote.is_pressed(event) {
            return Some(VimTextObject::SingleQuote);
        }
        if self.vim_text_object_keymap.backtick.is_pressed(event) {
            return Some(VimTextObject::Backtick);
        }
        None
    }

    pub(super) fn text_object_range(
        &self,
        object: VimTextObject,
        scope: VimTextObjectScope,
    ) -> Option<Range<usize>> {
        match object {
            VimTextObject::Word => self.word_text_object_range(scope, /*big_word*/ false),
            VimTextObject::BigWord => self.word_text_object_range(scope, /*big_word*/ true),
            VimTextObject::Parentheses => self.paired_text_object_range(scope, '(', ')'),
            VimTextObject::Brackets => self.paired_text_object_range(scope, '[', ']'),
            VimTextObject::Braces => self.paired_text_object_range(scope, '{', '}'),
            VimTextObject::DoubleQuote => self.quoted_text_object_range(scope, '"'),
            VimTextObject::SingleQuote => self.quoted_text_object_range(scope, '\''),
            VimTextObject::Backtick => self.quoted_text_object_range(scope, '`'),
        }
    }

    fn word_text_object_range(
        &self,
        scope: VimTextObjectScope,
        big_word: bool,
    ) -> Option<Range<usize>> {
        let inner = if big_word {
            self.big_word_range_at_cursor()?
        } else {
            self.small_word_range_at_cursor()?
        };
        Some(match scope {
            VimTextObjectScope::Inner => inner,
            VimTextObjectScope::Around => self.expand_word_around(inner),
        })
    }

    fn big_word_range_at_cursor(&self) -> Option<Range<usize>> {
        self.non_ws_runs()
            .into_iter()
            .find(|range| self.cursor_overlaps_range(range) || self.cursor_is_at_range_end(range))
    }

    fn small_word_range_at_cursor(&self) -> Option<Range<usize>> {
        for run in self.non_ws_runs() {
            if !self.cursor_overlaps_range(&run) && !self.cursor_is_at_range_end(&run) {
                continue;
            }
            let mut last_piece = None;
            for (piece_start, piece) in split_word_pieces(&self.text[run.clone()]) {
                let piece = run.start + piece_start..run.start + piece_start + piece.len();
                if self.cursor_overlaps_range(&piece) {
                    return Some(piece);
                }
                last_piece = Some(piece);
            }
            if self.cursor_is_at_range_end(&run) {
                return last_piece.or(Some(run));
            }
            return Some(run);
        }
        None
    }

    fn non_ws_runs(&self) -> Vec<Range<usize>> {
        let mut runs = Vec::new();
        let mut start = None;
        for (idx, ch) in self.text.char_indices() {
            if ch.is_whitespace() {
                if let Some(run_start) = start.take() {
                    runs.push(run_start..idx);
                }
            } else if start.is_none() {
                start = Some(idx);
            }
        }
        if let Some(run_start) = start {
            runs.push(run_start..self.text.len());
        }
        runs
    }

    fn cursor_overlaps_range(&self, range: &Range<usize>) -> bool {
        range.start <= self.cursor_pos && self.cursor_pos < range.end
    }

    fn cursor_is_at_range_end(&self, range: &Range<usize>) -> bool {
        range.start < range.end && self.cursor_pos == range.end
    }

    fn expand_word_around(&self, inner: Range<usize>) -> Range<usize> {
        let following = self.following_whitespace_end(inner.end);
        if following > inner.end {
            return inner.start..following;
        }
        self.preceding_whitespace_start(inner.start)..inner.end
    }

    fn following_whitespace_end(&self, start: usize) -> usize {
        let mut end = start;
        for (offset, ch) in self.text[start..].char_indices() {
            if !ch.is_whitespace() {
                break;
            }
            end = start + offset + ch.len_utf8();
        }
        end
    }

    fn preceding_whitespace_start(&self, end: usize) -> usize {
        let mut start = end;
        for (idx, ch) in self.text[..end].char_indices().rev() {
            if !ch.is_whitespace() {
                break;
            }
            start = idx;
        }
        start
    }

    fn paired_text_object_range(
        &self,
        scope: VimTextObjectScope,
        open: char,
        close: char,
    ) -> Option<Range<usize>> {
        let mut stack: Vec<usize> = Vec::new();
        let mut best: Option<Range<usize>> = None;
        for (idx, ch) in self.text.char_indices() {
            if self.is_inside_element(idx) {
                continue;
            }
            if ch == open {
                stack.push(idx);
            } else if ch == close {
                let Some(open_idx) = stack.pop() else {
                    continue;
                };
                let close_end = idx + ch.len_utf8();
                if open_idx <= self.cursor_pos && self.cursor_pos <= idx {
                    let candidate = match scope {
                        VimTextObjectScope::Inner => open_idx + open.len_utf8()..idx,
                        VimTextObjectScope::Around => open_idx..close_end,
                    };
                    if candidate.start <= candidate.end
                        && best
                            .as_ref()
                            .is_none_or(|current| candidate.len() < current.len())
                    {
                        best = Some(candidate);
                    }
                }
            }
        }
        best
    }

    fn quoted_text_object_range(
        &self,
        scope: VimTextObjectScope,
        quote: char,
    ) -> Option<Range<usize>> {
        let line = self.beginning_of_current_line()..self.end_of_current_line();
        let mut open = None;
        let mut best: Option<Range<usize>> = None;
        for (offset, ch) in self.text[line.clone()].char_indices() {
            let idx = line.start + offset;
            if self.is_inside_element(idx) || ch != quote || self.is_escaped(idx) {
                continue;
            }
            if let Some(open_idx) = open.take() {
                if open_idx <= self.cursor_pos && self.cursor_pos <= idx {
                    let candidate = match scope {
                        VimTextObjectScope::Inner => open_idx + quote.len_utf8()..idx,
                        VimTextObjectScope::Around => idx_range(open_idx, idx, quote),
                    };
                    if candidate.start <= candidate.end
                        && best
                            .as_ref()
                            .is_none_or(|current| candidate.len() < current.len())
                    {
                        best = Some(candidate);
                    }
                }
            } else {
                open = Some(idx);
            }
        }
        best
    }

    fn is_inside_element(&self, pos: usize) -> bool {
        self.elements
            .iter()
            .any(|element| pos >= element.range.start && pos < element.range.end)
    }

    fn is_escaped(&self, pos: usize) -> bool {
        let mut backslashes = 0;
        for ch in self.text[..pos].chars().rev() {
            if ch != '\\' {
                break;
            }
            backslashes += 1;
        }
        backslashes % 2 == 1
    }
}

fn idx_range(open_idx: usize, close_idx: usize, quote: char) -> Range<usize> {
    open_idx..close_idx + quote.len_utf8()
}
