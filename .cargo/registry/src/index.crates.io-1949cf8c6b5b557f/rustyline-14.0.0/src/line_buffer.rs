//! Line buffer with current cursor position
use crate::keymap::{At, CharSearch, Movement, RepeatCount, Word};
use std::cmp::min;
use std::fmt;
use std::iter;
use std::ops::{Deref, Index, Range};
use std::string::Drain;
use unicode_segmentation::UnicodeSegmentation;

/// Default maximum buffer size for the line read
pub(crate) const MAX_LINE: usize = 4096;
pub(crate) const INDENT: &str = "                                ";

/// Word's case change
#[derive(Clone, Copy)]
pub enum WordAction {
    /// Capitalize word
    Capitalize,
    /// lowercase word
    Lowercase,
    /// uppercase word
    Uppercase,
}

/// Delete (kill) direction
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// After cursor
    #[default]
    Forward,
    /// Before cursor
    Backward,
}

/// Listener to be notified when some text is deleted.
pub trait DeleteListener {
    /// used to make the distinction between simple character(s) deletion and
    /// word(s)/line(s) deletion
    fn start_killing(&mut self) {}
    /// `string` deleted at `idx` index
    fn delete(&mut self, idx: usize, string: &str, dir: Direction);
    /// used to make the distinction between simple character(s) deletion and
    /// word(s)/line(s) deletion
    fn stop_killing(&mut self) {}
}

/// Listener to be notified when the line is modified.
pub trait ChangeListener: DeleteListener {
    /// `c`har inserted at `idx` index
    fn insert_char(&mut self, idx: usize, c: char);
    /// `string` inserted at `idx` index
    fn insert_str(&mut self, idx: usize, string: &str);
    /// `old` text replaced by `new` text at `idx` index
    fn replace(&mut self, idx: usize, old: &str, new: &str);
}

pub(crate) struct NoListener;

impl DeleteListener for NoListener {
    fn delete(&mut self, _idx: usize, _string: &str, _dir: Direction) {}
}
impl ChangeListener for NoListener {
    fn insert_char(&mut self, _idx: usize, _c: char) {}

    fn insert_str(&mut self, _idx: usize, _string: &str) {}

    fn replace(&mut self, _idx: usize, _old: &str, _new: &str) {}
}

// TODO split / cache lines ?

/// Represent the current input (text and cursor position).
///
/// The methods do text manipulations or/and cursor movements.
pub struct LineBuffer {
    buf: String,      // Edited line buffer (rl_line_buffer)
    pos: usize,       // Current cursor position (byte position) (rl_point)
    can_growth: bool, // Whether to allow dynamic growth
}

impl fmt::Debug for LineBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineBuffer")
            .field("buf", &self.buf)
            .field("pos", &self.pos)
            .finish()
    }
}

impl LineBuffer {
    /// Create a new line buffer with the given maximum `capacity`.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: String::with_capacity(capacity),
            pos: 0,
            can_growth: false,
        }
    }

    /// Set whether to allow dynamic allocation
    pub(crate) fn can_growth(mut self, can_growth: bool) -> Self {
        self.can_growth = can_growth;
        self
    }

    fn must_truncate(&self, new_len: usize) -> bool {
        !self.can_growth && new_len > self.buf.capacity()
    }

    #[cfg(test)]
    pub(crate) fn init(line: &str, pos: usize) -> Self {
        let mut lb = Self::with_capacity(MAX_LINE);
        assert!(lb.insert_str(0, line, &mut NoListener));
        lb.set_pos(pos);
        lb
    }

    /// Extracts a string slice containing the entire buffer.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// Converts a buffer into a `String` without copying or allocating.
    #[must_use]
    pub fn into_string(self) -> String {
        self.buf
    }

    /// Current cursor position (byte position)
    #[must_use]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Set cursor position (byte position)
    pub fn set_pos(&mut self, pos: usize) {
        assert!(pos <= self.buf.len());
        self.pos = pos;
    }

    /// Returns the length of this buffer, in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if this buffer has a length of zero.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Set line content (`buf`) and cursor position (`pos`).
    pub fn update<C: ChangeListener>(&mut self, buf: &str, pos: usize, cl: &mut C) {
        assert!(pos <= buf.len());
        let end = self.len();
        self.drain(0..end, Direction::default(), cl);
        let max = self.buf.capacity();
        if self.must_truncate(buf.len()) {
            self.insert_str(0, &buf[..max], cl);
            if pos > max {
                self.pos = max;
            } else {
                self.pos = pos;
            }
        } else {
            self.insert_str(0, buf, cl);
            self.pos = pos;
        }
    }

    fn end_of_line(&self) -> usize {
        if let Some(n) = self.buf[self.pos..].find('\n') {
            n + self.pos
        } else {
            self.buf.len()
        }
    }

    fn start_of_line(&self) -> usize {
        if let Some(i) = self.buf[..self.pos].rfind('\n') {
            // `i` is before the new line, e.g. at the end of the previous one.
            i + 1
        } else {
            0
        }
    }

    /// Returns the character at current cursor position.
    pub(crate) fn grapheme_at_cursor(&self) -> Option<&str> {
        if self.pos == self.buf.len() {
            None
        } else {
            self.buf[self.pos..].graphemes(true).next()
        }
    }

    /// Returns the position of the character just after the current cursor
    /// position.
    #[must_use]
    pub fn next_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .take(n)
            .last()
            .map(|(i, s)| i + self.pos + s.len())
    }

    /// Returns the position of the character just before the current cursor
    /// position.
    fn prev_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == 0 {
            return None;
        }
        self.buf[..self.pos]
            .grapheme_indices(true)
            .rev()
            .take(n)
            .last()
            .map(|(i, _)| i)
    }

    /// Insert the character `ch` at current cursor position
    /// and advance cursor position accordingly.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn insert<C: ChangeListener>(
        &mut self,
        ch: char,
        n: RepeatCount,
        cl: &mut C,
    ) -> Option<bool> {
        let shift = ch.len_utf8() * n;
        if self.must_truncate(self.buf.len() + shift) {
            return None;
        }
        let push = self.pos == self.buf.len();
        if n == 1 {
            self.buf.insert(self.pos, ch);
            cl.insert_char(self.pos, ch);
        } else {
            let text = iter::repeat(ch).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text, cl);
        }
        self.pos += shift;
        Some(push)
    }

    /// Yank/paste `text` at current position.
    /// Return `None` when maximum buffer size has been reached or is empty,
    /// `true` when the character has been appended to the end of the line.
    pub fn yank<C: ChangeListener>(
        &mut self,
        text: &str,
        n: RepeatCount,
        cl: &mut C,
    ) -> Option<bool> {
        let shift = text.len() * n;
        if text.is_empty() || self.must_truncate(self.buf.len() + shift) {
            return None;
        }
        let push = self.pos == self.buf.len();
        let pos = self.pos;
        if n == 1 {
            self.insert_str(pos, text, cl);
        } else {
            let text = text.repeat(n);
            self.insert_str(pos, &text, cl);
        }
        self.pos += shift;
        Some(push)
    }

    /// Delete previously yanked text and yank/paste `text` at current position.
    pub fn yank_pop<C: ChangeListener>(
        &mut self,
        yank_size: usize,
        text: &str,
        cl: &mut C,
    ) -> Option<bool> {
        let end = self.pos;
        let start = end - yank_size;
        self.drain(start..end, Direction::default(), cl);
        self.pos -= yank_size;
        self.yank(text, 1, cl)
    }

    /// Move cursor on the left.
    pub fn move_backward(&mut self, n: RepeatCount) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor on the right.
    pub fn move_forward(&mut self, n: RepeatCount) -> bool {
        match self.next_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor to the start of the buffer.
    pub fn move_buffer_start(&mut self) -> bool {
        if self.pos > 0 {
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the buffer.
    pub fn move_buffer_end(&mut self) -> bool {
        if self.pos == self.buf.len() {
            false
        } else {
            self.pos = self.buf.len();
            true
        }
    }

    /// Move cursor to the start of the line.
    pub fn move_home(&mut self) -> bool {
        let start = self.start_of_line();
        if self.pos > start {
            self.pos = start;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) -> bool {
        let end = self.end_of_line();
        if self.pos == end {
            false
        } else {
            self.pos = end;
            true
        }
    }

    /// Is cursor at the end of input (whitespaces after cursor is discarded)
    #[must_use]
    pub fn is_end_of_input(&self) -> bool {
        self.pos >= self.buf.trim_end().len()
    }

    /// Delete the character at the right of the cursor without altering the
    /// cursor position. Basically this is what happens with the "Delete"
    /// keyboard key.
    /// Return the number of characters deleted.
    pub fn delete<D: DeleteListener>(&mut self, n: RepeatCount, dl: &mut D) -> Option<String> {
        match self.next_pos(n) {
            Some(pos) => {
                let start = self.pos;
                let chars = self
                    .drain(start..pos, Direction::Forward, dl)
                    .collect::<String>();
                Some(chars)
            }
            None => None,
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace<D: DeleteListener>(&mut self, n: RepeatCount, dl: &mut D) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                let end = self.pos;
                self.drain(pos..end, Direction::Backward, dl);
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Kill the text from point to the end of the line.
    pub fn kill_line<D: DeleteListener>(&mut self, dl: &mut D) -> bool {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let start = self.pos;
            let end = self.end_of_line();
            if start == end {
                self.delete(1, dl);
            } else {
                self.drain(start..end, Direction::Forward, dl);
            }
            true
        } else {
            false
        }
    }

    /// Kill the text from point to the end of the buffer.
    pub fn kill_buffer<D: DeleteListener>(&mut self, dl: &mut D) -> bool {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let start = self.pos;
            let end = self.buf.len();
            self.drain(start..end, Direction::Forward, dl);
            true
        } else {
            false
        }
    }

    /// Kill backward from point to the beginning of the line.
    pub fn discard_line<D: DeleteListener>(&mut self, dl: &mut D) -> bool {
        if self.pos > 0 && !self.buf.is_empty() {
            let start = self.start_of_line();
            let end = self.pos;
            if end == start {
                self.backspace(1, dl)
            } else {
                self.drain(start..end, Direction::Backward, dl);
                self.pos = start;
                true
            }
        } else {
            false
        }
    }

    /// Kill backward from point to the beginning of the buffer.
    pub fn discard_buffer<D: DeleteListener>(&mut self, dl: &mut D) -> bool {
        if self.pos > 0 && !self.buf.is_empty() {
            let end = self.pos;
            self.drain(0..end, Direction::Backward, dl);
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Exchange the char before cursor with the character at cursor.
    pub fn transpose_chars<C: ChangeListener>(&mut self, cl: &mut C) -> bool {
        if self.pos == 0 || self.buf.graphemes(true).count() < 2 {
            return false;
        }
        if self.pos == self.buf.len() {
            self.move_backward(1);
        }
        let chars = self.delete(1, cl).unwrap();
        self.move_backward(1);
        self.yank(&chars, 1, cl);
        self.move_forward(1);
        true
    }

    /// Go left until start of word
    fn prev_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == 0 {
            return None;
        }
        let mut sow = 0;
        let mut gis = self.buf[..pos].grapheme_indices(true).rev();
        'outer: for _ in 0..n {
            sow = 0;
            let mut gj = gis.next();
            'inner: loop {
                if let Some((j, y)) = gj {
                    let gi = gis.next();
                    if let Some((_, x)) = gi {
                        if is_start_of_word(word_def, x, y) {
                            sow = j;
                            break 'inner;
                        }
                        gj = gi;
                    } else {
                        break 'outer;
                    }
                } else {
                    break 'outer;
                }
            }
        }
        Some(sow)
    }

    /// Moves the cursor to the beginning of previous word.
    pub fn move_to_prev_word(&mut self, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Delete the previous word, maintaining the cursor at the start of the
    /// current word.
    pub fn delete_prev_word<D: DeleteListener>(
        &mut self,
        word_def: Word,
        n: RepeatCount,
        dl: &mut D,
    ) -> bool {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            let end = self.pos;
            self.drain(pos..end, Direction::Backward, dl);
            self.pos = pos;
            true
        } else {
            false
        }
    }

    fn next_word_pos(&self, pos: usize, at: At, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == self.buf.len() {
            return None;
        }
        let mut wp = 0;
        let mut gis = self.buf[pos..].grapheme_indices(true);
        let mut gi = if at == At::BeforeEnd {
            // TODO Validate
            gis.next()
        } else {
            None
        };
        'outer: for _ in 0..n {
            wp = 0;
            gi = gis.next();
            'inner: loop {
                if let Some((i, x)) = gi {
                    let gj = gis.next();
                    if let Some((j, y)) = gj {
                        if at == At::Start && is_start_of_word(word_def, x, y) {
                            wp = j;
                            break 'inner;
                        } else if at != At::Start && is_end_of_word(word_def, x, y) {
                            if word_def == Word::Emacs || at == At::AfterEnd {
                                wp = j;
                            } else {
                                wp = i;
                            }
                            break 'inner;
                        }
                        gi = gj;
                    } else {
                        break 'outer;
                    }
                } else {
                    break 'outer;
                }
            }
        }
        if wp == 0 {
            if word_def == Word::Emacs || at == At::AfterEnd {
                Some(self.buf.len())
            } else {
                match gi {
                    Some((i, _)) if i != 0 => Some(i + pos),
                    _ => None,
                }
            }
        } else {
            Some(wp + pos)
        }
    }

    /// Moves the cursor to the end of next word.
    pub fn move_to_next_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Moves the cursor to the same column in the line above
    pub fn move_to_line_up(&mut self, n: RepeatCount) -> bool {
        match self.buf[..self.pos].rfind('\n') {
            Some(off) => {
                let column = self.buf[off + 1..self.pos].graphemes(true).count();

                let mut dest_start = self.buf[..off].rfind('\n').map_or(0, |n| n + 1);
                let mut dest_end = off;
                for _ in 1..n {
                    if dest_start == 0 {
                        break;
                    }
                    dest_end = dest_start - 1;
                    dest_start = self.buf[..dest_end].rfind('\n').map_or(0, |n| n + 1);
                }
                let gidx = self.buf[dest_start..dest_end]
                    .grapheme_indices(true)
                    .nth(column);

                self.pos = gidx.map_or(off, |(idx, _)| dest_start + idx); // if there's no enough columns
                true
            }
            None => false,
        }
    }

    /// N lines up starting from the current one
    ///
    /// Fails if the cursor is on the first line
    fn n_lines_up(&self, n: RepeatCount) -> Option<(usize, usize)> {
        let mut start = if let Some(off) = self.buf[..self.pos].rfind('\n') {
            off + 1
        } else {
            return None;
        };
        let end = self.buf[self.pos..]
            .find('\n')
            .map_or_else(|| self.buf.len(), |x| self.pos + x + 1);
        for _ in 0..n {
            if let Some(off) = self.buf[..start - 1].rfind('\n') {
                start = off + 1;
            } else {
                start = 0;
                break;
            }
        }
        Some((start, end))
    }

    /// N lines down starting from the current one
    ///
    /// Fails if the cursor is on the last line
    fn n_lines_down(&self, n: RepeatCount) -> Option<(usize, usize)> {
        let mut end = if let Some(off) = self.buf[self.pos..].find('\n') {
            self.pos + off + 1
        } else {
            return None;
        };
        let start = self.buf[..self.pos].rfind('\n').unwrap_or(0);
        for _ in 0..n {
            if let Some(off) = self.buf[end..].find('\n') {
                end = end + off + 1;
            } else {
                end = self.buf.len();
                break;
            };
        }
        Some((start, end))
    }

    /// Moves the cursor to the same column in the line above
    pub fn move_to_line_down(&mut self, n: RepeatCount) -> bool {
        match self.buf[self.pos..].find('\n') {
            Some(off) => {
                let line_start = self.buf[..self.pos].rfind('\n').map_or(0, |n| n + 1);
                let column = self.buf[line_start..self.pos].graphemes(true).count();
                let mut dest_start = self.pos + off + 1;
                let mut dest_end = self.buf[dest_start..]
                    .find('\n')
                    .map_or_else(|| self.buf.len(), |v| dest_start + v);
                for _ in 1..n {
                    if dest_end == self.buf.len() {
                        break;
                    }
                    dest_start = dest_end + 1;
                    dest_end = self.buf[dest_start..]
                        .find('\n')
                        .map_or_else(|| self.buf.len(), |v| dest_start + v);
                }
                self.pos = self.buf[dest_start..dest_end]
                    .grapheme_indices(true)
                    .nth(column)
                    .map_or(dest_end, |(idx, _)| dest_start + idx); // if there's no enough columns
                debug_assert!(self.pos <= self.buf.len());
                true
            }
            None => false,
        }
    }

    fn search_char_pos(&self, cs: CharSearch, n: RepeatCount) -> Option<usize> {
        let mut shift = 0;
        let search_result = match cs {
            CharSearch::Backward(c) | CharSearch::BackwardAfter(c) => self.buf[..self.pos]
                .char_indices()
                .rev()
                .filter(|&(_, ch)| ch == c)
                .take(n)
                .last()
                .map(|(i, _)| i),
            CharSearch::Forward(c) | CharSearch::ForwardBefore(c) => {
                if let Some(cc) = self.grapheme_at_cursor() {
                    shift = self.pos + cc.len();
                    if shift < self.buf.len() {
                        self.buf[shift..]
                            .char_indices()
                            .filter(|&(_, ch)| ch == c)
                            .take(n)
                            .last()
                            .map(|(i, _)| i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };
        search_result.map(|pos| match cs {
            CharSearch::Backward(_) => pos,
            CharSearch::BackwardAfter(c) => pos + c.len_utf8(),
            CharSearch::Forward(_) => shift + pos,
            CharSearch::ForwardBefore(_) => {
                shift + pos
                    - self.buf[..shift + pos]
                        .chars()
                        .next_back()
                        .unwrap()
                        .len_utf8()
            }
        })
    }

    /// Move cursor to the matching character position.
    /// Return `true` when the search succeeds.
    pub fn move_to(&mut self, cs: CharSearch, n: RepeatCount) -> bool {
        if let Some(pos) = self.search_char_pos(cs, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Kill from the cursor to the end of the current word,
    /// or, if between words, to the end of the next word.
    pub fn delete_word<D: DeleteListener>(
        &mut self,
        at: At,
        word_def: Word,
        n: RepeatCount,
        dl: &mut D,
    ) -> bool {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            let start = self.pos;
            self.drain(start..pos, Direction::Forward, dl);
            true
        } else {
            false
        }
    }

    /// Delete range specified by `cs` search.
    pub fn delete_to<D: DeleteListener>(
        &mut self,
        cs: CharSearch,
        n: RepeatCount,
        dl: &mut D,
    ) -> bool {
        let search_result = match cs {
            CharSearch::ForwardBefore(c) => self.search_char_pos(CharSearch::Forward(c), n),
            _ => self.search_char_pos(cs, n),
        };
        if let Some(pos) = search_result {
            match cs {
                CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                    let end = self.pos;
                    self.pos = pos;
                    self.drain(pos..end, Direction::Backward, dl);
                }
                CharSearch::ForwardBefore(_) => {
                    let start = self.pos;
                    self.drain(start..pos, Direction::Forward, dl);
                }
                CharSearch::Forward(c) => {
                    let start = self.pos;
                    self.drain(start..pos + c.len_utf8(), Direction::Forward, dl);
                }
            };
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&self) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .find_map(|(i, ch)| {
                if ch.chars().all(char::is_alphanumeric) {
                    Some(i)
                } else {
                    None
                }
            })
            .map(|i| i + self.pos)
    }

    /// Alter the next word.
    pub fn edit_word<C: ChangeListener>(&mut self, a: WordAction, cl: &mut C) -> bool {
        if let Some(start) = self.skip_whitespace() {
            if let Some(end) = self.next_word_pos(start, At::AfterEnd, Word::Emacs, 1) {
                if start == end {
                    return false;
                }
                let word = self
                    .drain(start..end, Direction::default(), cl)
                    .collect::<String>();
                let result = match a {
                    WordAction::Capitalize => {
                        let ch = word.graphemes(true).next().unwrap();
                        let cap = ch.to_uppercase();
                        cap + &word[ch.len()..].to_lowercase()
                    }
                    WordAction::Lowercase => word.to_lowercase(),
                    WordAction::Uppercase => word.to_uppercase(),
                };
                self.insert_str(start, &result, cl);
                self.pos = start + result.len();
                return true;
            }
        }
        false
    }

    /// Transpose two words
    pub fn transpose_words<C: ChangeListener>(&mut self, n: RepeatCount, cl: &mut C) -> bool {
        let word_def = Word::Emacs;
        self.move_to_next_word(At::AfterEnd, word_def, n);
        let w2_end = self.pos;
        self.move_to_prev_word(word_def, 1);
        let w2_beg = self.pos;
        self.move_to_prev_word(word_def, n);
        let w1_beg = self.pos;
        self.move_to_next_word(At::AfterEnd, word_def, 1);
        let w1_end = self.pos;
        if w1_beg == w2_beg || w2_beg < w1_end {
            return false;
        }

        let w1 = self.buf[w1_beg..w1_end].to_owned();

        let w2 = self
            .drain(w2_beg..w2_end, Direction::default(), cl)
            .collect::<String>();
        self.insert_str(w2_beg, &w1, cl);

        self.drain(w1_beg..w1_end, Direction::default(), cl);
        self.insert_str(w1_beg, &w2, cl);

        self.pos = w2_end;
        true
    }

    /// Replaces the content between [`start`..`end`] with `text`
    /// and positions the cursor to the end of text.
    pub fn replace<C: ChangeListener>(&mut self, range: Range<usize>, text: &str, cl: &mut C) {
        let start = range.start;
        cl.replace(start, self.buf.index(range.clone()), text);
        self.buf.drain(range);
        if start == self.buf.len() {
            self.buf.push_str(text);
        } else {
            self.buf.insert_str(start, text);
        }
        self.pos = start + text.len();
    }

    /// Insert the `s`tring at the specified position.
    /// Return `true` if the text has been inserted at the end of the line.
    pub fn insert_str<C: ChangeListener>(&mut self, idx: usize, s: &str, cl: &mut C) -> bool {
        cl.insert_str(idx, s);
        if idx == self.buf.len() {
            self.buf.push_str(s);
            true
        } else {
            self.buf.insert_str(idx, s);
            false
        }
    }

    /// Remove the specified `range` in the line.
    pub fn delete_range<D: DeleteListener>(&mut self, range: Range<usize>, dl: &mut D) {
        self.set_pos(range.start);
        self.drain(range, Direction::default(), dl);
    }

    fn drain<D: DeleteListener>(
        &mut self,
        range: Range<usize>,
        dir: Direction,
        dl: &mut D,
    ) -> Drain<'_> {
        dl.delete(range.start, &self.buf[range.start..range.end], dir);
        self.buf.drain(range)
    }

    /// Return the content between current cursor position and `mvt` position.
    /// Return `None` when the buffer is empty or when the movement fails.
    #[must_use]
    pub fn copy(&self, mvt: &Movement) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        match *mvt {
            Movement::WholeLine => {
                let start = self.start_of_line();
                let end = self.end_of_line();
                if start == end {
                    None
                } else {
                    Some(self.buf[start..self.pos].to_owned())
                }
            }
            Movement::BeginningOfLine => {
                let start = self.start_of_line();
                if self.pos == start {
                    None
                } else {
                    Some(self.buf[start..self.pos].to_owned())
                }
            }
            Movement::ViFirstPrint => {
                if self.pos == 0 {
                    None
                } else {
                    self.next_word_pos(0, At::Start, Word::Big, 1)
                        .map(|pos| self.buf[pos..self.pos].to_owned())
                }
            }
            Movement::EndOfLine => {
                let end = self.end_of_line();
                if self.pos == end {
                    None
                } else {
                    Some(self.buf[self.pos..end].to_owned())
                }
            }
            Movement::EndOfBuffer => {
                if self.pos == self.buf.len() {
                    None
                } else {
                    Some(self.buf[self.pos..].to_owned())
                }
            }
            Movement::WholeBuffer => {
                if self.buf.is_empty() {
                    None
                } else {
                    Some(self.buf.clone())
                }
            }
            Movement::BeginningOfBuffer => {
                if self.pos == 0 {
                    None
                } else {
                    Some(self.buf[..self.pos].to_owned())
                }
            }
            Movement::BackwardWord(n, word_def) => self
                .prev_word_pos(self.pos, word_def, n)
                .map(|pos| self.buf[pos..self.pos].to_owned()),
            Movement::ForwardWord(n, at, word_def) => self
                .next_word_pos(self.pos, at, word_def, n)
                .map(|pos| self.buf[self.pos..pos].to_owned()),
            Movement::ViCharSearch(n, cs) => {
                let search_result = match cs {
                    CharSearch::ForwardBefore(c) => self.search_char_pos(CharSearch::Forward(c), n),
                    _ => self.search_char_pos(cs, n),
                };
                search_result.map(|pos| match cs {
                    CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                        self.buf[pos..self.pos].to_owned()
                    }
                    CharSearch::ForwardBefore(_) => self.buf[self.pos..pos].to_owned(),
                    CharSearch::Forward(c) => self.buf[self.pos..pos + c.len_utf8()].to_owned(),
                })
            }
            Movement::BackwardChar(n) => self
                .prev_pos(n)
                .map(|pos| self.buf[pos..self.pos].to_owned()),
            Movement::ForwardChar(n) => self
                .next_pos(n)
                .map(|pos| self.buf[self.pos..pos].to_owned()),
            Movement::LineUp(n) => {
                if let Some((start, end)) = self.n_lines_up(n) {
                    Some(self.buf[start..end].to_owned())
                } else {
                    None
                }
            }
            Movement::LineDown(n) => {
                if let Some((start, end)) = self.n_lines_down(n) {
                    Some(self.buf[start..end].to_owned())
                } else {
                    None
                }
            }
        }
    }

    /// Kill range specified by `mvt`.
    pub fn kill<D: DeleteListener>(&mut self, mvt: &Movement, dl: &mut D) -> bool {
        let notify = !matches!(*mvt, Movement::ForwardChar(_) | Movement::BackwardChar(_));
        if notify {
            dl.start_killing();
        }
        let killed = match *mvt {
            Movement::ForwardChar(n) => {
                // Delete (forward) `n` characters at point.
                self.delete(n, dl).is_some()
            }
            Movement::BackwardChar(n) => {
                // Delete `n` characters backward.
                self.backspace(n, dl)
            }
            Movement::EndOfLine => {
                // Kill the text from point to the end of the line.
                self.kill_line(dl)
            }
            Movement::WholeLine => {
                self.move_home();
                self.kill_line(dl)
            }
            Movement::BeginningOfLine => {
                // Kill backward from point to the beginning of the line.
                self.discard_line(dl)
            }
            Movement::BackwardWord(n, word_def) => {
                // kill `n` words backward (until start of word)
                self.delete_prev_word(word_def, n, dl)
            }
            Movement::ForwardWord(n, at, word_def) => {
                // kill `n` words forward (until start/end of word)
                self.delete_word(at, word_def, n, dl)
            }
            Movement::ViCharSearch(n, cs) => self.delete_to(cs, n, dl),
            Movement::LineUp(n) => {
                if let Some((start, end)) = self.n_lines_up(n) {
                    self.delete_range(start..end, dl);
                    true
                } else {
                    false
                }
            }
            Movement::LineDown(n) => {
                if let Some((start, end)) = self.n_lines_down(n) {
                    self.delete_range(start..end, dl);
                    true
                } else {
                    false
                }
            }
            Movement::ViFirstPrint => {
                false // TODO
            }
            Movement::EndOfBuffer => {
                // Kill the text from point to the end of the buffer.
                self.kill_buffer(dl)
            }
            Movement::BeginningOfBuffer => {
                // Kill backward from point to the beginning of the buffer.
                self.discard_buffer(dl)
            }
            Movement::WholeBuffer => {
                self.move_buffer_start();
                self.kill_buffer(dl)
            }
        };
        if notify {
            dl.stop_killing();
        }
        killed
    }

    /// Indent range specified by `mvt`.
    pub fn indent<C: ChangeListener>(
        &mut self,
        mvt: &Movement,
        amount: usize,
        dedent: bool,
        cl: &mut C,
    ) -> bool {
        let pair = match *mvt {
            // All inline operators are the same: indent current line
            Movement::WholeLine
            | Movement::BeginningOfLine
            | Movement::ViFirstPrint
            | Movement::EndOfLine
            | Movement::BackwardChar(..)
            | Movement::ForwardChar(..)
            | Movement::ViCharSearch(..) => Some((self.pos, self.pos)),
            Movement::EndOfBuffer => Some((self.pos, self.buf.len())),
            Movement::WholeBuffer => Some((0, self.buf.len())),
            Movement::BeginningOfBuffer => Some((0, self.pos)),
            Movement::BackwardWord(n, word_def) => self
                .prev_word_pos(self.pos, word_def, n)
                .map(|pos| (pos, self.pos)),
            Movement::ForwardWord(n, at, word_def) => self
                .next_word_pos(self.pos, at, word_def, n)
                .map(|pos| (self.pos, pos)),
            Movement::LineUp(n) => self.n_lines_up(n),
            Movement::LineDown(n) => self.n_lines_down(n),
        };
        let (start, end) = pair.unwrap_or((self.pos, self.pos));
        let start = self.buf[..start].rfind('\n').map_or(0, |pos| pos + 1);
        let end = self.buf[end..]
            .rfind('\n')
            .map_or_else(|| self.buf.len(), |pos| end + pos);
        let mut index = start;
        if dedent {
            for line in self.buf[start..end].to_string().split('\n') {
                let max = line.len() - line.trim_start().len();
                let deleting = min(max, amount);
                self.drain(index..index + deleting, Direction::default(), cl);
                if self.pos >= index {
                    if self.pos.saturating_sub(index) < deleting {
                        // don't wrap into the previous line
                        self.pos = index;
                    } else {
                        self.pos -= deleting;
                    }
                }
                index += line.len() + 1 - deleting;
            }
        } else {
            for line in self.buf[start..end].to_string().split('\n') {
                for off in (0..amount).step_by(INDENT.len()) {
                    self.insert_str(index, &INDENT[..min(amount - off, INDENT.len())], cl);
                }
                if self.pos >= index {
                    self.pos += amount;
                }
                index += amount + line.len() + 1;
            }
        }
        true
    }
}

impl Deref for LineBuffer {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

fn is_start_of_word(word_def: Word, previous: &str, grapheme: &str) -> bool {
    (!is_word_char(word_def, previous) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(previous) && is_other_char(grapheme))
}
fn is_end_of_word(word_def: Word, grapheme: &str, next: &str) -> bool {
    (!is_word_char(word_def, next) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(next) && is_other_char(grapheme))
}

fn is_word_char(word_def: Word, grapheme: &str) -> bool {
    match word_def {
        Word::Emacs => grapheme.chars().all(char::is_alphanumeric),
        Word::Vi => is_vi_word_char(grapheme),
        Word::Big => !grapheme.chars().any(char::is_whitespace),
    }
}
fn is_vi_word_char(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_alphanumeric) || grapheme == "_"
}
fn is_other_char(grapheme: &str) -> bool {
    !(grapheme.chars().any(char::is_whitespace) || is_vi_word_char(grapheme))
}

#[cfg(test)]
mod test {
    use super::{
        ChangeListener, DeleteListener, Direction, LineBuffer, NoListener, WordAction, MAX_LINE,
    };
    use crate::keymap::{At, CharSearch, Word};

    struct Listener {
        deleted_str: Option<String>,
    }

    impl Listener {
        fn new() -> Listener {
            Listener { deleted_str: None }
        }

        fn assert_deleted_str_eq(&self, expected: &str) {
            let actual = self.deleted_str.as_ref().expect("no deleted string");
            assert_eq!(expected, actual)
        }
    }

    impl DeleteListener for Listener {
        fn delete(&mut self, _: usize, string: &str, _: Direction) {
            self.deleted_str = Some(string.to_owned());
        }
    }
    impl ChangeListener for Listener {
        fn insert_char(&mut self, _: usize, _: char) {}

        fn insert_str(&mut self, _: usize, _: &str) {}

        fn replace(&mut self, _: usize, _: &str, _: &str) {}
    }

    #[test]
    fn next_pos() {
        let s = LineBuffer::init("ö̲g̈", 0);
        assert_eq!(7, s.len());
        let pos = s.next_pos(1);
        assert_eq!(Some(4), pos);

        let s = LineBuffer::init("ö̲g̈", 4);
        let pos = s.next_pos(1);
        assert_eq!(Some(7), pos);
    }

    #[test]
    fn prev_pos() {
        let s = LineBuffer::init("ö̲g̈", 4);
        assert_eq!(7, s.len());
        let pos = s.prev_pos(1);
        assert_eq!(Some(0), pos);

        let s = LineBuffer::init("ö̲g̈", 7);
        let pos = s.prev_pos(1);
        assert_eq!(Some(4), pos);
    }

    #[test]
    fn insert() {
        let mut s = LineBuffer::with_capacity(MAX_LINE);
        let push = s.insert('α', 1, &mut NoListener).unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert!(push);

        let push = s.insert('ß', 1, &mut NoListener).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert!(push);

        s.pos = 0;
        let push = s.insert('γ', 1, &mut NoListener).unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
        assert!(!push);
    }

    #[test]
    fn yank_after() {
        let mut s = LineBuffer::init("αß", 2);
        s.move_forward(1);
        let ok = s.yank("γδε", 1, &mut NoListener);
        assert_eq!(Some(true), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(10, s.pos);
    }

    #[test]
    fn yank_before() {
        let mut s = LineBuffer::init("αε", 2);
        let ok = s.yank("ßγδ", 1, &mut NoListener);
        assert_eq!(Some(false), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn moves() {
        let mut s = LineBuffer::init("αß", 4);
        let ok = s.move_backward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);
        assert!(ok);

        let ok = s.move_forward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);

        let ok = s.move_home();
        assert_eq!("αß", s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);

        let ok = s.move_end();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);
    }

    #[test]
    fn move_home_end_multiline() {
        let text = "αa\nsdf ßc\nasdf";
        let mut s = LineBuffer::init(text, 7);
        let ok = s.move_home();
        assert_eq!(text, s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);

        let ok = s.move_home();
        assert_eq!(text, s.buf);
        assert_eq!(4, s.pos);
        assert!(!ok);

        let ok = s.move_end();
        assert_eq!(text, s.buf);
        assert_eq!(11, s.pos);
        assert!(ok);

        let ok = s.move_end();
        assert_eq!(text, s.buf);
        assert_eq!(11, s.pos);
        assert!(!ok);
    }

    #[test]
    fn move_buffer_multiline() {
        let text = "αa\nsdf ßc\nasdf";
        let mut s = LineBuffer::init(text, 7);
        let ok = s.move_buffer_start();
        assert_eq!(text, s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);

        let ok = s.move_buffer_start();
        assert_eq!(text, s.buf);
        assert_eq!(0, s.pos);
        assert!(!ok);

        let ok = s.move_buffer_end();
        assert_eq!(text, s.buf);
        assert_eq!(text.len(), s.pos);
        assert!(ok);

        let ok = s.move_buffer_end();
        assert_eq!(text, s.buf);
        assert_eq!(text.len(), s.pos);
        assert!(!ok);
    }

    #[test]
    fn move_grapheme() {
        let mut s = LineBuffer::init("ag̈", 4);
        assert_eq!(4, s.len());
        let ok = s.move_backward(1);
        assert!(ok);
        assert_eq!(1, s.pos);

        let ok = s.move_forward(1);
        assert!(ok);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αß", 2);
        let chars = s.delete(1, &mut cl);
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß".to_owned()), chars);

        let ok = s.backspace(1, &mut cl);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("α");
    }

    #[test]
    fn kill() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 6);
        let ok = s.kill_line(&mut cl);
        assert_eq!("αßγ", s.buf);
        assert_eq!(6, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("δε");

        s.pos = 4;
        let ok = s.discard_line(&mut cl);
        assert_eq!("γ", s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("αß");
    }

    #[test]
    fn kill_multiline() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αß\nγδ 12\nε f4", 7);

        let ok = s.kill_line(&mut cl);
        assert_eq!("αß\nγ\nε f4", s.buf);
        assert_eq!(7, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("δ 12");

        let ok = s.kill_line(&mut cl);
        assert_eq!("αß\nγε f4", s.buf);
        assert_eq!(7, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("\n");

        let ok = s.kill_line(&mut cl);
        assert_eq!("αß\nγ", s.buf);
        assert_eq!(7, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("ε f4");

        let ok = s.kill_line(&mut cl);
        assert_eq!(7, s.pos);
        assert!(!ok);
    }

    #[test]
    fn discard_multiline() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αß\nc γδε", 9);

        let ok = s.discard_line(&mut cl);
        assert_eq!("αß\nδε", s.buf);
        assert_eq!(5, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("c γ");

        let ok = s.discard_line(&mut cl);
        assert_eq!("αßδε", s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("\n");

        let ok = s.discard_line(&mut cl);
        assert_eq!("δε", s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("αß");

        let ok = s.discard_line(&mut cl);
        assert_eq!(0, s.pos);
        assert!(!ok);
    }

    #[test]
    fn transpose() {
        let mut s = LineBuffer::init("aßc", 1);
        let ok = s.transpose_chars(&mut NoListener);
        assert_eq!("ßac", s.buf);
        assert_eq!(3, s.pos);
        assert!(ok);

        s.buf = String::from("aßc");
        s.pos = 3;
        let ok = s.transpose_chars(&mut NoListener);
        assert_eq!("acß", s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);

        s.buf = String::from("aßc");
        s.pos = 4;
        let ok = s.transpose_chars(&mut NoListener);
        assert_eq!("acß", s.buf);
        assert_eq!(4, s.pos);
        assert!(ok);
    }

    #[test]
    fn move_to_prev_word() {
        let mut s = LineBuffer::init("a ß  c", 6); // before 'c'
        let ok = s.move_to_prev_word(Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(2, s.pos); // before 'ß'
        assert!(ok);

        assert!(s.move_end()); // after 'c'
        assert_eq!(7, s.pos);
        let ok = s.move_to_prev_word(Word::Emacs, 1);
        assert!(ok);
        assert_eq!(6, s.pos); // before 'c'

        let ok = s.move_to_prev_word(Word::Emacs, 2);
        assert!(ok);
        assert_eq!(0, s.pos);
    }

    #[test]
    fn move_to_prev_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(12, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(7, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(0, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_prev_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(0, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_forward() {
        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.move_to(CharSearch::ForwardBefore('ε'), 1);
        assert!(ok);
        assert_eq!(6, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.move_to(CharSearch::Forward('ε'), 1);
        assert!(ok);
        assert_eq!(8, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.move_to(CharSearch::Forward('ε'), 10);
        assert!(ok);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn move_to_backward() {
        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.move_to(CharSearch::BackwardAfter('ß'), 1);
        assert!(ok);
        assert_eq!(4, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.move_to(CharSearch::Backward('ß'), 1);
        assert!(ok);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_prev_word() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 6);
        let ok = s.delete_prev_word(Word::Big, 1, &mut cl);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("ß  ");
    }

    #[test]
    fn move_to_next_word() {
        let mut s = LineBuffer::init("a ß  c", 1); // after 'a'
        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert!(ok);
        assert_eq!(4, s.pos); // after 'ß'

        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert!(ok);
        assert_eq!(7, s.pos); // after 'c'

        s.move_home();
        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert!(ok);
        assert_eq!(1, s.pos); // after 'a'

        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 2);
        assert!(ok);
        assert_eq!(7, s.pos); // after 'c'
    }

    #[test]
    fn move_to_end_of_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(6, s.pos);
        assert!(ok);
    }

    #[test]
    fn move_to_end_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(10, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(14, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_end_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_word() {
        let mut s = LineBuffer::init("a ß  c", 2);
        let ok = s.move_to_next_word(At::Start, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(6, s.pos);
        assert!(ok);
    }

    #[test]
    fn move_to_start_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(7, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(12, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn delete_word() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 1);
        let ok = s.delete_word(At::AfterEnd, Word::Emacs, 1, &mut cl);
        assert_eq!("a  c", s.buf);
        assert_eq!(1, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq(" ß");

        let mut s = LineBuffer::init("test", 0);
        let ok = s.delete_word(At::AfterEnd, Word::Vi, 1, &mut cl);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("test");
    }

    #[test]
    fn delete_til_start_of_word() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 2);
        let ok = s.delete_word(At::Start, Word::Emacs, 1, &mut cl);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert!(ok);
        cl.assert_deleted_str_eq("ß  ");
    }

    #[test]
    fn delete_to_forward() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.delete_to(CharSearch::ForwardBefore('ε'), 1, &mut cl);
        assert!(ok);
        cl.assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.delete_to(CharSearch::Forward('ε'), 1, &mut cl);
        assert!(ok);
        cl.assert_deleted_str_eq("ßγδε");
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_to_backward() {
        let mut cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.delete_to(CharSearch::BackwardAfter('α'), 1, &mut cl);
        assert!(ok);
        cl.assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.delete_to(CharSearch::Backward('ß'), 1, &mut cl);
        assert!(ok);
        cl.assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn edit_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1);
        assert!(s.edit_word(WordAction::Uppercase, &mut NoListener));
        assert_eq!("a SSETA  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßetA  c", 1);
        assert!(s.edit_word(WordAction::Lowercase, &mut NoListener));
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßETA  c", 1);
        assert!(s.edit_word(WordAction::Capitalize, &mut NoListener));
        assert_eq!("a SSeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("test", 1);
        assert!(s.edit_word(WordAction::Capitalize, &mut NoListener));
        assert_eq!("tEst", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn transpose_words() {
        let mut s = LineBuffer::init("ßeta / δelta__", 15);
        assert!(s.transpose_words(1, &mut NoListener));
        assert_eq!("δelta__ / ßeta", s.buf);
        assert_eq!(16, s.pos);

        let mut s = LineBuffer::init("ßeta / δelta", 14);
        assert!(s.transpose_words(1, &mut NoListener));
        assert_eq!("δelta / ßeta", s.buf);
        assert_eq!(14, s.pos);

        let mut s = LineBuffer::init(" / δelta", 8);
        assert!(!s.transpose_words(1, &mut NoListener));

        let mut s = LineBuffer::init("ßeta / __", 9);
        assert!(!s.transpose_words(1, &mut NoListener));
    }

    #[test]
    fn move_by_line() {
        let text = "aa123\nsdf bc\nasdf";
        let mut s = LineBuffer::init(text, 14);
        // move up
        let ok = s.move_to_line_up(1);
        assert_eq!(7, s.pos);
        assert!(ok);

        let ok = s.move_to_line_up(1);
        assert_eq!(1, s.pos);
        assert!(ok);

        let ok = s.move_to_line_up(1);
        assert_eq!(1, s.pos);
        assert!(!ok);

        // move down
        let ok = s.move_to_line_down(1);
        assert_eq!(7, s.pos);
        assert!(ok);

        let ok = s.move_to_line_down(1);
        assert_eq!(14, s.pos);
        assert!(ok);

        let ok = s.move_to_line_down(1);
        assert_eq!(14, s.pos);
        assert!(!ok);

        // move by multiple steps
        let ok = s.move_to_line_up(2);
        assert_eq!(1, s.pos);
        assert!(ok);

        let ok = s.move_to_line_down(2);
        assert_eq!(14, s.pos);
        assert!(ok);
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LineBuffer>();
    }

    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LineBuffer>();
    }
}
