use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::WidgetRef;
use std::cell::Ref;
use std::cell::RefCell;
use std::ops::Range;
use textwrap::Options;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimState {
    Insert,
    Normal,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimPendingOperator {
    Delete,
    Change,
    Yank,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct VimRecordedChange {
    start: usize,
    end: usize,
    inserted: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimPendingFind {
    ForwardInclusive,
    ForwardExclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimFindDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VimFindState {
    target: char,
    direction: VimFindDirection,
    stop_before: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VimPendingTextObject {
    Inner,
    Around,
}

#[derive(Debug, Clone)]
struct TextElement {
    range: Range<usize>,
}

#[derive(Debug, Clone)]
struct TextAreaSnapshot {
    text: String,
    cursor: usize,
    elements: Vec<TextElement>,
    preferred_col: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct TextArea {
    text: String,
    cursor_pos: usize,
    wrap_cache: RefCell<Option<WrapCache>>,
    preferred_col: Option<usize>,
    elements: Vec<TextElement>,
    // Vim mode support
    vim_mode_enabled: bool,
    vim_state: VimState,
    vim_pending_op: Option<VimPendingOperator>,
    vim_input_count: Option<usize>,
    vim_last_change: Option<VimRecordedChange>,
    vim_pending_find: Option<VimPendingFind>,
    vim_pending_g: bool,
    vim_last_find: Option<VimFindState>,
    vim_register: String,
    undo_stack: Vec<TextAreaSnapshot>,
    redo_stack: Vec<TextAreaSnapshot>,
    suspend_undo: bool,
    vim_pending_textobj: Option<VimPendingTextObject>,
    vim_pending_replace: bool,
    vim_last_command: Vec<KeyEvent>,
    vim_current_command: Vec<KeyEvent>,
    vim_recording_command: bool,
    vim_replaying_dot: bool,
    vim_pending_count_keys: Vec<KeyEvent>,
    vim_last_unhandled: Option<KeyEvent>,
}

#[derive(Debug, Clone)]
struct WrapCache {
    width: u16,
    lines: Vec<Range<usize>>,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct TextAreaState {
    /// Index into wrapped lines of the first visible line.
    scroll: u16,
}

impl TextArea {
    const UNDO_STACK_LIMIT: usize = 256;
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            wrap_cache: RefCell::new(None),
            preferred_col: None,
            elements: Vec::new(),
            vim_mode_enabled: false,
            vim_state: VimState::Insert,
            vim_pending_op: None,
            vim_input_count: None,
            vim_last_change: None,
            vim_pending_find: None,
            vim_pending_g: false,
            vim_last_find: None,
            vim_register: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            suspend_undo: false,
            vim_pending_textobj: None,
            vim_pending_replace: false,
            vim_last_command: Vec::new(),
            vim_current_command: Vec::new(),
            vim_recording_command: false,
            vim_replaying_dot: false,
            vim_pending_count_keys: Vec::new(),
            vim_last_unhandled: None,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len());
        self.wrap_cache.replace(None);
        self.preferred_col = None;
        self.elements.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.suspend_undo = false;
        self.vim_pending_textobj = None;
        self.vim_pending_replace = false;
        self.vim_recording_command = false;
        self.vim_pending_count_keys.clear();
        self.vim_current_command.clear();
        self.vim_last_unhandled = None;
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert_str(&mut self, text: &str) {
        self.insert_str_at(self.cursor_pos, text);
    }

    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        self.push_undo_state();
        let pos = self.clamp_pos_for_insertion(pos);
        self.text.insert_str(pos, text);
        self.wrap_cache.replace(None);
        if pos <= self.cursor_pos {
            self.cursor_pos += text.len();
        }
        self.shift_elements(pos, 0, text.len());
        self.preferred_col = None;
    }

    pub fn replace_range(&mut self, range: std::ops::Range<usize>, text: &str) {
        let range = self.expand_range_to_element_boundaries(range);
        self.replace_range_raw(range, text);
    }

    fn replace_range_raw(&mut self, range: std::ops::Range<usize>, text: &str) {
        assert!(range.start <= range.end);
        let start = range.start.clamp(0, self.text.len());
        let end = range.end.clamp(0, self.text.len());
        let removed_len = end - start;
        let inserted_len = text.len();
        self.push_undo_state();
        if removed_len == 0 && inserted_len == 0 {
            return;
        }
        let diff = inserted_len as isize - removed_len as isize;

        self.text.replace_range(range, text);
        self.wrap_cache.replace(None);
        self.preferred_col = None;
        self.update_elements_after_replace(start, end, inserted_len);

        // Update the cursor position to account for the edit.
        self.cursor_pos = if self.cursor_pos < start {
            // Cursor was before the edited range – no shift.
            self.cursor_pos
        } else if self.cursor_pos <= end {
            // Cursor was inside the replaced range – move to end of the new text.
            start + inserted_len
        } else {
            // Cursor was after the replaced range – shift by the length diff.
            ((self.cursor_pos as isize) + diff) as usize
        }
        .min(self.text.len());

        // Ensure cursor is not inside an element
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    pub fn cursor(&self) -> usize {
        self.cursor_pos
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor_pos = pos.clamp(0, self.text.len());
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.wrapped_lines(width).len() as u16
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_with_state(area, TextAreaState::default())
    }

    /// Compute the on-screen cursor position taking scrolling into account.
    pub fn cursor_pos_with_state(&self, area: Rect, state: TextAreaState) -> Option<(u16, u16)> {
        let lines = self.wrapped_lines(area.width);
        let effective_scroll = self.effective_scroll(area.height, &lines, state.scroll);
        let i = Self::wrapped_line_index_by_start(&lines, self.cursor_pos)?;
        let ls = &lines[i];
        let col = self.text[ls.start..self.cursor_pos].width() as u16;
        let screen_row = i
            .saturating_sub(effective_scroll as usize)
            .try_into()
            .unwrap_or(0);
        Some((area.x + col, area.y + screen_row))
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn current_display_col(&self) -> usize {
        let bol = self.beginning_of_current_line();
        self.text[bol..self.cursor_pos].width()
    }

    fn wrapped_line_index_by_start(lines: &[Range<usize>], pos: usize) -> Option<usize> {
        // partition_point returns the index of the first element for which
        // the predicate is false, i.e. the count of elements with start <= pos.
        let idx = lines.partition_point(|r| r.start <= pos);
        if idx == 0 { None } else { Some(idx - 1) }
    }

    fn move_to_display_col_on_line(
        &mut self,
        line_start: usize,
        line_end: usize,
        target_col: usize,
    ) {
        let mut width_so_far = 0usize;
        for (i, g) in self.text[line_start..line_end].grapheme_indices(true) {
            width_so_far += g.width();
            if width_so_far > target_col {
                self.cursor_pos = line_start + i;
                // Avoid landing inside an element; round to nearest boundary
                self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
                return;
            }
        }
        self.cursor_pos = line_end;
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    fn beginning_of_line(&self, pos: usize) -> usize {
        self.text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }
    fn beginning_of_current_line(&self) -> usize {
        self.beginning_of_line(self.cursor_pos)
    }

    fn end_of_line(&self, pos: usize) -> usize {
        self.text[pos..]
            .find('\n')
            .map(|i| i + pos)
            .unwrap_or(self.text.len())
    }
    fn end_of_current_line(&self) -> usize {
        self.end_of_line(self.cursor_pos)
    }

    fn vim_clear_count(&mut self) {
        self.vim_input_count = None;
    }

    fn vim_take_count(&mut self) -> usize {
        self.vim_input_count.take().unwrap_or(1).max(1)
    }

    fn vim_append_digit(&mut self, digit: u32) {
        let base = self.vim_input_count.unwrap_or(0);
        let next = base
            .saturating_mul(10)
            .saturating_add(digit as usize)
            .max(1);
        self.vim_input_count = Some(next);
    }

    fn mark_unhandled_vim_key(&mut self, event: KeyEvent) {
        self.vim_last_unhandled = Some(event);
    }

    pub(crate) fn take_last_unhandled_vim_key(&mut self) -> Option<KeyEvent> {
        self.vim_last_unhandled.take()
    }

    fn vim_total_lines(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        }
    }

    fn vim_line_start_for(&self, line_idx: usize) -> usize {
        if line_idx == 0 {
            return 0;
        }
        let mut seen = 0usize;
        for (i, c) in self.text.char_indices() {
            if c == '\n' {
                seen += 1;
                if seen == line_idx {
                    return (i + 1).min(self.text.len());
                }
            }
        }
        self.text.len()
    }

    fn vim_current_line_index(&self) -> usize {
        self.text[..self.cursor_pos]
            .chars()
            .filter(|&c| c == '\n')
            .count()
    }

    fn vim_move_cursor_to_line(&mut self, line_idx: usize, first_non_blank: bool) {
        let total_lines = self.vim_total_lines();
        if total_lines == 0 {
            self.set_cursor(0);
            return;
        }
        let clamped = line_idx.min(total_lines.saturating_sub(1));
        let start = self.vim_line_start_for(clamped);
        let end = self.end_of_line(start);
        let target = if first_non_blank {
            self.vim_first_non_blank(start, end)
        } else {
            start
        };
        self.set_cursor(target);
    }

    fn vim_first_non_blank(&self, start: usize, end: usize) -> usize {
        for (idx, ch) in self.text[start..end].char_indices() {
            if !ch.is_whitespace() {
                return start + idx;
            }
        }
        start
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn is_word_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_'
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn clamp_to_char_boundary(&self, pos: usize) -> usize {
        if pos >= self.text.len() {
            return self.text.len();
        }
        if self.text.is_char_boundary(pos) {
            pos
        } else {
            self.prev_atomic_boundary(pos)
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn expand_word_at(&self, start: usize) -> (usize, usize) {
        let mut start = start;
        while start > 0 {
            let prev = self.prev_atomic_boundary(start);
            if let Some(ch) = self.text[prev..].chars().next()
                && Self::is_word_char(ch)
            {
                start = prev;
                continue;
            }
            break;
        }
        let mut end = self.next_atomic_boundary(start);
        while end < self.text.len() {
            if let Some(ch) = self.text[end..].chars().next()
                && Self::is_word_char(ch)
            {
                end = self.next_atomic_boundary(end);
                continue;
            }
            break;
        }
        (
            self.adjust_pos_out_of_elements(start, true),
            self.adjust_pos_out_of_elements(end, false),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn find_word_region(&self, pos: usize) -> Option<(usize, usize)> {
        if self.text.is_empty() {
            return None;
        }
        let len = self.text.len();
        let mut pos = pos.min(len);
        pos = self.clamp_to_char_boundary(pos);

        if pos < len
            && let Some(ch) = self.text[pos..].chars().next()
            && Self::is_word_char(ch)
        {
            return Some(self.expand_word_at(pos));
        }

        if pos > 0 {
            let prev = self.prev_atomic_boundary(pos);
            if let Some(ch) = self.text[prev..].chars().next()
                && Self::is_word_char(ch)
            {
                return Some(self.expand_word_at(prev));
            }
        }

        let mut iter = pos;
        while iter < len {
            iter = self.next_atomic_boundary(iter);
            if iter >= len {
                break;
            }
            if let Some(ch) = self.text[iter..].chars().next()
                && Self::is_word_char(ch)
            {
                return Some(self.expand_word_at(iter));
            }
        }
        None
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn vim_inner_word_range(&self) -> Option<(usize, usize)> {
        let (start, end) = self.find_word_region(self.cursor_pos)?;
        if start == end {
            None
        } else {
            Some((start, end))
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn vim_around_word_range(&self) -> Option<(usize, usize)> {
        let (start, inner_end) = self.vim_inner_word_range()?;
        let mut cursor = inner_end;
        let mut end = inner_end;
        while cursor < self.text.len() {
            let ch = self.text[cursor..].chars().next().unwrap();
            if ch == ' ' || ch == '\t' {
                cursor = self.next_atomic_boundary(cursor);
                end = cursor;
            } else {
                break;
            }
        }
        Some((start, end))
    }

    fn vim_enter_insert_mode(&mut self) {
        self.vim_state = VimState::Insert;
        self.vim_pending_op = None;
        self.vim_pending_find = None;
        self.vim_pending_g = false;
        self.vim_pending_textobj = None;
        self.vim_pending_replace = false;
        self.vim_clear_count();
    }

    fn begin_vim_command(&mut self, event: KeyEvent) {
        if self.vim_replaying_dot {
            return;
        }
        if !self.vim_recording_command {
            self.vim_recording_command = true;
            self.vim_current_command.clear();
            if !self.vim_pending_count_keys.is_empty() {
                self.vim_current_command
                    .extend(self.vim_pending_count_keys.iter().copied());
                self.vim_pending_count_keys.clear();
            }
        }
        self.vim_current_command.push(event);
    }

    fn push_vim_command_event(&mut self, event: KeyEvent) {
        if self.vim_replaying_dot {
            return;
        }
        if self.vim_recording_command {
            self.vim_current_command.push(event);
        }
    }

    fn finish_vim_command(&mut self) {
        if self.vim_replaying_dot {
            return;
        }
        if self.vim_recording_command {
            if !self.vim_current_command.is_empty() {
                self.vim_last_command = self.vim_current_command.clone();
            }
            self.vim_recording_command = false;
            self.vim_current_command.clear();
            self.vim_pending_count_keys.clear();
        }
    }

    fn abort_vim_command(&mut self) {
        if self.vim_replaying_dot {
            return;
        }
        self.vim_recording_command = false;
        self.vim_current_command.clear();
        self.vim_pending_count_keys.clear();
    }

    fn record_count_key(&mut self, event: KeyEvent) {
        if self.vim_replaying_dot {
            return;
        }
        if self.vim_recording_command {
            self.vim_current_command.push(event);
        } else {
            self.vim_pending_count_keys.push(event);
        }
    }

    fn replay_last_command(&mut self) {
        if self.vim_last_command.is_empty() {
            return;
        }
        self.vim_replaying_dot = true;
        let cmds = self.vim_last_command.clone();
        for evt in cmds {
            self.input(evt);
        }
        self.vim_replaying_dot = false;
    }

    fn vim_store_range(&mut self, start: usize, end: usize) {
        if end <= start || start >= self.text.len() {
            return;
        }
        let end = end.min(self.text.len());
        self.vim_register = self.text[start..end].to_string();
    }

    fn snapshot(&self) -> TextAreaSnapshot {
        TextAreaSnapshot {
            text: self.text.clone(),
            cursor: self.cursor_pos,
            elements: self.elements.clone(),
            preferred_col: self.preferred_col,
        }
    }

    fn push_undo_state(&mut self) {
        if self.suspend_undo {
            return;
        }
        let snapshot = self.snapshot();
        if self
            .undo_stack
            .last()
            .is_some_and(|last| last.text == snapshot.text && last.cursor == snapshot.cursor)
        {
            return;
        }
        if self.undo_stack.len() >= Self::UNDO_STACK_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
    }

    fn apply_snapshot(&mut self, snapshot: TextAreaSnapshot) {
        self.suspend_undo = true;
        self.text = snapshot.text;
        self.cursor_pos = snapshot.cursor.min(self.text.len());
        self.elements = snapshot.elements;
        self.preferred_col = snapshot.preferred_col;
        self.wrap_cache.replace(None);
        self.suspend_undo = false;
        self.vim_state = VimState::Normal;
        self.vim_pending_op = None;
        self.vim_pending_find = None;
        self.vim_pending_g = false;
        self.vim_pending_textobj = None;
        self.vim_clear_count();
    }

    fn vim_undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = self.snapshot();
            if self.redo_stack.len() >= Self::UNDO_STACK_LIMIT {
                self.redo_stack.remove(0);
            }
            self.redo_stack.push(current);
            self.apply_snapshot(snapshot);
        }
    }

    fn vim_redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = self.snapshot();
            if self.undo_stack.len() >= Self::UNDO_STACK_LIMIT {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(current);
            self.apply_snapshot(snapshot);
        }
    }

    fn vim_change_forward_to(&mut self, end: usize) {
        self.vim_delete_forward_to(end);
        self.vim_enter_insert_mode();
    }

    fn vim_change_current_line(&mut self, count: usize) {
        let count = count.max(1);
        let start = self.beginning_of_current_line();
        let mut cursor = start;
        let mut end = start;
        for _ in 0..count {
            let line_end = self.end_of_line(cursor);
            end = line_end;
            if end < self.text.len() {
                end += 1;
            }
            cursor = end;
        }
        end = end.min(self.text.len());
        self.vim_store_range(start, end);
        self.replace_range(start..end, "");
        self.insert_str_at(start, "\n");
        self.set_cursor(start);
        self.vim_enter_insert_mode();
    }

    fn vim_yank_lines(&mut self, count: usize) {
        let count = count.max(1);
        let start = self.beginning_of_current_line();
        let mut cursor = start;
        let mut end = start;
        for _ in 0..count {
            let line_end = self.end_of_line(cursor);
            end = line_end;
            if end < self.text.len() {
                end += 1;
            }
            cursor = end;
        }
        end = end.min(self.text.len());
        self.vim_store_range(start, end);
        self.set_cursor(start);
    }

    fn vim_replace_with_char(&mut self, ch: char) {
        let count = self.vim_take_count().max(1);
        let start = self.cursor_pos;
        let mut end = start;
        for _ in 0..count {
            let next = self.next_atomic_boundary(end);
            if next == end {
                break;
            }
            end = next;
        }
        if end <= start {
            return;
        }
        let inserted = ch.to_string().repeat(count);
        self.replace_range(start..end, &inserted);
        let char_len = ch.len_utf8();
        let new_cursor = start + char_len.saturating_mul(count.saturating_sub(1));
        self.set_cursor(new_cursor);
        self.vim_last_change = Some(VimRecordedChange {
            start,
            end: start + inserted.len(),
            inserted,
        });
    }

    #[allow(dead_code)]
    fn replay_last_change(&mut self) {
        if let Some(change) = self.vim_last_change.clone() {
            let VimRecordedChange {
                start,
                end,
                inserted,
            } = change;
            let end = end.min(self.text.len());
            self.replace_range(start..end, &inserted);
            self.set_cursor(start + inserted.len());
        }
    }

    fn char_len_at(&self, pos: usize) -> usize {
        self.text[pos..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0)
    }

    fn vim_find_forward_match(&self, from: usize, target: char, count: usize) -> Option<usize> {
        let line_end = self.end_of_line(from);
        if from >= line_end {
            return None;
        }
        let slice = &self.text[from..line_end];
        let mut remaining = count;
        for (offset, ch) in slice.char_indices() {
            if ch == target {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    return Some(from + offset);
                }
            }
        }
        None
    }

    fn vim_find_backward_match(&self, from: usize, target: char, count: usize) -> Option<usize> {
        let line_start = self.beginning_of_line(from);
        if from <= line_start {
            return None;
        }
        let mut remaining = count;
        let mut pos = self.prev_atomic_boundary(from);
        while pos >= line_start {
            if let Some(ch) = self.text[pos..].chars().next()
                && ch == target
            {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    return Some(pos);
                }
            }
            if pos == line_start {
                break;
            }
            let next_pos = self.prev_atomic_boundary(pos);
            if next_pos == pos {
                break;
            }
            pos = next_pos;
        }
        None
    }

    fn vim_perform_find(
        &mut self,
        target: char,
        direction: VimFindDirection,
        stop_before: bool,
        count: usize,
    ) -> bool {
        let count = count.max(1);
        let result = match direction {
            VimFindDirection::Forward => {
                let start = self.next_atomic_boundary(self.cursor_pos);
                self.vim_find_forward_match(start, target, count)
            }
            VimFindDirection::Backward => {
                self.vim_find_backward_match(self.cursor_pos, target, count)
            }
        };

        let Some(found) = result else {
            if self.vim_pending_op.is_some() {
                self.vim_pending_op = None;
                self.abort_vim_command();
            }
            return false;
        };

        let char_len = self.char_len_at(found);
        let dest = match (direction, stop_before) {
            (VimFindDirection::Forward, false) => found,
            (VimFindDirection::Forward, true) => {
                let prev = self.prev_atomic_boundary(found);
                if prev == found {
                    return false;
                }
                prev
            }
            (VimFindDirection::Backward, false) => found,
            (VimFindDirection::Backward, true) => {
                let after = self.next_atomic_boundary(found + char_len);
                after.min(self.text.len())
            }
        };

        if let Some(op) = self.vim_pending_op.take() {
            match op {
                VimPendingOperator::Delete => {
                    let delete_end = match direction {
                        VimFindDirection::Forward => {
                            if stop_before {
                                self.next_atomic_boundary(dest)
                            } else {
                                self.next_atomic_boundary(found + char_len)
                            }
                        }
                        VimFindDirection::Backward => {
                            // Deleting backwards is not yet supported; bail out and restore state.
                            self.vim_pending_op = Some(VimPendingOperator::Delete);
                            return false;
                        }
                    };
                    if delete_end <= self.cursor_pos {
                        return false;
                    }
                    self.vim_delete_forward_to(delete_end);
                }
                VimPendingOperator::Change => {
                    let delete_end = match direction {
                        VimFindDirection::Forward => {
                            if stop_before {
                                self.next_atomic_boundary(dest)
                            } else {
                                self.next_atomic_boundary(found + char_len)
                            }
                        }
                        VimFindDirection::Backward => {
                            self.vim_pending_op = Some(VimPendingOperator::Change);
                            return false;
                        }
                    };
                    if delete_end <= self.cursor_pos {
                        return false;
                    }
                    self.vim_change_forward_to(delete_end);
                }
                VimPendingOperator::Yank => {
                    self.vim_pending_op = Some(VimPendingOperator::Yank);
                    self.vim_clear_count();
                    return false;
                }
            }
            // Operator motions leave cursor at the beginning (already true).
        } else {
            self.set_cursor(dest);
        }

        self.vim_last_find = Some(VimFindState {
            target,
            direction,
            stop_before,
        });
        true
    }

    fn vim_register_is_linewise(&self) -> bool {
        self.vim_register.contains('\n')
    }

    fn vim_put_after(&mut self, count: usize) {
        if self.vim_register.is_empty() {
            return;
        }
        let times = count.max(1);
        let insertion = self.vim_register.repeat(times);
        if self.vim_register_is_linewise() {
            let end = self.end_of_current_line();
            let insert_pos = if end < self.text.len() {
                end + 1
            } else {
                self.text.len()
            };
            self.insert_str_at(insert_pos, &insertion);
            self.set_cursor(insert_pos);
        } else {
            let mut pos = self.next_atomic_boundary(self.cursor_pos);
            self.insert_str_at(pos, &insertion);
            pos += insertion.len();
            self.set_cursor(pos);
        }
    }

    fn vim_put_before(&mut self, count: usize) {
        if self.vim_register.is_empty() {
            return;
        }
        let times = count.max(1);
        let insertion = self.vim_register.repeat(times);
        if self.vim_register_is_linewise() {
            let insert_pos = self.beginning_of_current_line();
            self.insert_str_at(insert_pos, &insertion);
            self.set_cursor(insert_pos);
        } else {
            let pos = self.cursor_pos;
            self.insert_str_at(pos, &insertion);
            self.set_cursor(pos + insertion.len());
        }
    }

    pub fn input(&mut self, event: KeyEvent) {
        self.vim_last_unhandled = None;
        // Handle Vim mode state machine first. When enabled and in Normal mode,
        // we consume most keystrokes here and return early.
        if self.vim_mode_enabled {
            // ESC (or Ctrl+[) always returns to Normal from either state.
            match event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                }
                | KeyEvent {
                    code: KeyCode::Char('['),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    if matches!(self.vim_state, VimState::Insert) {
                        if self.vim_recording_command {
                            self.push_vim_command_event(event);
                            self.finish_vim_command();
                        }
                    } else if self.vim_recording_command {
                        self.abort_vim_command();
                    }
                    self.vim_state = VimState::Normal;
                    self.preferred_col = None;
                    self.vim_pending_op = None;
                    self.vim_input_count = None;
                    self.vim_pending_find = None;
                    self.vim_pending_g = false;
                    self.vim_pending_textobj = None;
                    self.vim_pending_replace = false;
                    return;
                }
                _ => {}
            }

            match self.vim_state {
                VimState::Normal => {
                    if let Some(obj) = self.vim_pending_textobj.take() {
                        if self.vim_recording_command {
                            self.push_vim_command_event(event);
                        }
                        if let KeyEvent {
                            code: KeyCode::Char('w'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } = event
                        {
                            let range = match obj {
                                VimPendingTextObject::Inner => self.vim_inner_word_range(),
                                VimPendingTextObject::Around => self.vim_around_word_range(),
                            };
                            if let Some((start, end)) = range {
                                match self.vim_pending_op.take() {
                                    Some(VimPendingOperator::Delete) => {
                                        self.vim_store_range(start, end);
                                        self.replace_range(start..end, "");
                                        self.set_cursor(start);
                                        self.finish_vim_command();
                                    }
                                    Some(VimPendingOperator::Change) => {
                                        self.vim_store_range(start, end);
                                        self.replace_range(start..end, "");
                                        self.set_cursor(start);
                                        self.vim_enter_insert_mode();
                                    }
                                    Some(VimPendingOperator::Yank) => {
                                        self.vim_store_range(start, end);
                                        self.vim_pending_op = None;
                                    }
                                    None => {}
                                }
                            }
                            return;
                        }
                        self.vim_pending_textobj = None;
                        self.mark_unhandled_vim_key(event);
                        return;
                    }

                    if self.vim_pending_replace {
                        if self.vim_recording_command {
                            self.push_vim_command_event(event);
                        }
                        self.vim_pending_replace = false;
                        match event {
                            KeyEvent {
                                code: KeyCode::Char(ch),
                                modifiers,
                                ..
                            } if !modifiers.contains(KeyModifiers::CONTROL)
                                && !modifiers.contains(KeyModifiers::ALT) =>
                            {
                                self.vim_replace_with_char(ch);
                                self.finish_vim_command();
                                return;
                            }
                            KeyEvent {
                                code: KeyCode::Enter,
                                ..
                            } => {
                                self.vim_replace_with_char('\n');
                                self.finish_vim_command();
                                return;
                            }
                            _ => {
                                self.mark_unhandled_vim_key(event);
                                self.abort_vim_command();
                                self.vim_pending_op = None;
                                self.vim_clear_count();
                                return;
                            }
                        }
                    }

                    if let Some(pending) = self.vim_pending_find {
                        match event {
                            KeyEvent {
                                code: KeyCode::Char(ch),
                                modifiers,
                                ..
                            } => {
                                if !(modifiers.contains(KeyModifiers::CONTROL)
                                    || modifiers.contains(KeyModifiers::ALT))
                                {
                                    self.vim_pending_find = None;
                                    let stop_before =
                                        matches!(pending, VimPendingFind::ForwardExclusive);
                                    let count = self.vim_take_count();
                                    let success = self.vim_perform_find(
                                        ch,
                                        VimFindDirection::Forward,
                                        stop_before,
                                        count,
                                    );
                                    if !success {
                                        self.vim_clear_count();
                                    }
                                    return;
                                }
                            }
                            _ => {
                                self.vim_pending_find = None;
                                self.mark_unhandled_vim_key(event);
                                return;
                            }
                        }
                        self.vim_pending_find = None;
                    }

                    if self.vim_pending_g {
                        self.vim_pending_g = false;
                        if let KeyEvent {
                            code: KeyCode::Char('g'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } = event
                        {
                            let count = self.vim_take_count();
                            let target_line = count.saturating_sub(1);
                            self.vim_move_cursor_to_line(target_line, true);
                            return;
                        }
                    }

                    match event {
                        KeyEvent {
                            code: KeyCode::Char('u'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            self.abort_vim_command();
                            self.vim_pending_op = None;
                            self.vim_pending_textobj = None;
                            self.vim_pending_find = None;
                            self.vim_pending_replace = false;
                            self.vim_undo();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('r'),
                            modifiers,
                            ..
                        } if modifiers == KeyModifiers::CONTROL => {
                            self.abort_vim_command();
                            self.vim_pending_op = None;
                            self.vim_pending_textobj = None;
                            self.vim_pending_find = None;
                            self.vim_pending_replace = false;
                            self.vim_redo();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('r'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            if self.vim_recording_command && !self.vim_pending_replace {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            self.vim_pending_op = None;
                            self.vim_pending_textobj = None;
                            self.vim_pending_replace = true;
                            return;
                        }

                        KeyEvent {
                            code: KeyCode::Char('.'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.vim_pending_op = None;
                            self.vim_pending_textobj = None;
                            self.vim_pending_find = None;
                            self.vim_pending_replace = false;
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.replay_last_command();
                            }
                            return;
                        }

                        KeyEvent {
                            code: KeyCode::Char(ch @ '1'..='9'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            self.record_count_key(event);
                            self.vim_append_digit(ch.to_digit(10).unwrap());
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('0'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } if self.vim_input_count.is_some() || self.vim_pending_op.is_some() => {
                            self.record_count_key(event);
                            self.vim_append_digit(0);
                            return;
                        }
                        // Motions that can complete an operator or move the cursor.
                        KeyEvent {
                            code: KeyCode::Char('w'),
                            ..
                        } => {
                            if let Some(op) = self.vim_pending_op {
                                if self.vim_recording_command {
                                    self.push_vim_command_event(event);
                                }
                                let count = self.vim_take_count();
                                match op {
                                    VimPendingOperator::Delete => {
                                        for _ in 0..count {
                                            let end = self.beginning_of_next_word();
                                            self.vim_delete_forward_to(end);
                                        }
                                        self.vim_pending_op = None;
                                        self.finish_vim_command();
                                    }
                                    VimPendingOperator::Change => {
                                        let orig = self.cursor_pos;
                                        let mut end = orig;
                                        for _ in 0..count {
                                            self.cursor_pos = end;
                                            end = self.beginning_of_next_word();
                                        }
                                        self.cursor_pos = orig;
                                        self.vim_change_forward_to(end);
                                    }
                                    VimPendingOperator::Yank => {
                                        let orig = self.cursor_pos;
                                        let mut end = orig;
                                        for _ in 0..count {
                                            self.cursor_pos = end;
                                            end = self.beginning_of_next_word();
                                        }
                                        self.cursor_pos = orig;
                                        self.vim_store_range(orig, end);
                                        self.vim_pending_op = None;
                                    }
                                }
                                return;
                            }
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                let p = self.beginning_of_next_word();
                                self.set_cursor(p);
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('e'),
                            ..
                        } => {
                            if let Some(op) = self.vim_pending_op {
                                if self.vim_recording_command {
                                    self.push_vim_command_event(event);
                                }
                                let count = self.vim_take_count();
                                match op {
                                    VimPendingOperator::Delete => {
                                        for _ in 0..count {
                                            let end = self.end_of_next_word();
                                            self.vim_delete_forward_to(end);
                                        }
                                        self.vim_pending_op = None;
                                        self.finish_vim_command();
                                    }
                                    VimPendingOperator::Change => {
                                        let orig = self.cursor_pos;
                                        let mut end = orig;
                                        for _ in 0..count {
                                            self.cursor_pos = end;
                                            end = self.end_of_next_word();
                                        }
                                        self.cursor_pos = orig;
                                        self.vim_change_forward_to(end);
                                    }
                                    VimPendingOperator::Yank => {
                                        let orig = self.cursor_pos;
                                        let mut end = orig;
                                        for _ in 0..count {
                                            self.cursor_pos = end;
                                            end = self.end_of_next_word();
                                        }
                                        self.cursor_pos = orig;
                                        self.vim_store_range(orig, end);
                                        self.vim_pending_op = None;
                                    }
                                }
                                return;
                            }
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                let p = self.end_of_next_word();
                                self.set_cursor(p);
                            }
                            return;
                        }
                        // Pending operators
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            ..
                        } => {
                            if self.vim_recording_command
                                && !matches!(self.vim_pending_op, Some(VimPendingOperator::Delete))
                            {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            // If already pending delete, 'dd' deletes current line.
                            if matches!(self.vim_pending_op, Some(VimPendingOperator::Delete)) {
                                let count = self.vim_take_count();
                                for _ in 0..count {
                                    self.vim_delete_current_line();
                                }
                                self.vim_pending_op = None;
                                self.finish_vim_command();
                            } else {
                                self.vim_pending_op = Some(VimPendingOperator::Delete);
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            ..
                        } => {
                            if self.vim_recording_command
                                && !matches!(self.vim_pending_op, Some(VimPendingOperator::Change))
                            {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            if matches!(self.vim_pending_op, Some(VimPendingOperator::Change)) {
                                let count = self.vim_take_count();
                                self.vim_change_current_line(count);
                            } else {
                                self.vim_pending_op = Some(VimPendingOperator::Change);
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('y'),
                            ..
                        } => {
                            if matches!(self.vim_pending_op, Some(VimPendingOperator::Yank)) {
                                let count = self.vim_take_count();
                                self.vim_yank_lines(count);
                                self.vim_pending_op = None;
                                return;
                            }
                            self.vim_pending_op = Some(VimPendingOperator::Yank);
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('Y'),
                            ..
                        } => {
                            let count = self.vim_take_count();
                            self.vim_yank_lines(count);
                            self.vim_pending_op = None;
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('C'),
                            ..
                        } => {
                            let count = self.vim_take_count();
                            let count = count.max(1);
                            for i in 0..count {
                                self.kill_to_end_of_line();
                                if i + 1 < count
                                    && self.cursor_pos < self.text.len()
                                    && self.text.as_bytes()[self.cursor_pos] == b'\n'
                                {
                                    self.delete_forward(1);
                                }
                            }
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('g'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            if self.vim_pending_op.is_some() {
                                self.vim_pending_op = None;
                                self.vim_clear_count();
                                return;
                            }
                            self.vim_pending_g = true;
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('G'),
                            ..
                        } => {
                            let total_lines = self.vim_total_lines();
                            if total_lines == 0 {
                                self.set_cursor(0);
                                return;
                            }
                            if let Some(op) = self.vim_pending_op {
                                let count = self.vim_take_count();
                                match op {
                                    VimPendingOperator::Delete => {
                                        let target = if count > 1 {
                                            count.saturating_sub(1).min(total_lines - 1)
                                        } else {
                                            total_lines - 1
                                        };
                                        let end_line_start = self.vim_line_start_for(target);
                                        let mut end = self.end_of_line(end_line_start);
                                        if end < self.text.len() {
                                            end += 1;
                                        }
                                        self.vim_delete_forward_to(end);
                                        self.vim_pending_op = None;
                                    }
                                    VimPendingOperator::Change => {
                                        let target = if count > 1 {
                                            count.saturating_sub(1).min(total_lines - 1)
                                        } else {
                                            total_lines - 1
                                        };
                                        let end_line_start = self.vim_line_start_for(target);
                                        let mut end = self.end_of_line(end_line_start);
                                        if end < self.text.len() {
                                            end += 1;
                                        }
                                        self.vim_change_forward_to(end);
                                    }
                                    VimPendingOperator::Yank => {
                                        let target = if count > 1 {
                                            count.saturating_sub(1).min(total_lines - 1)
                                        } else {
                                            total_lines - 1
                                        };
                                        let start = self.beginning_of_current_line();
                                        let end_line_start = self.vim_line_start_for(target);
                                        let mut end = self.end_of_line(end_line_start);
                                        if end < self.text.len() {
                                            end += 1;
                                        }
                                        self.vim_store_range(start, end);
                                        self.vim_pending_op = None;
                                    }
                                }
                                return;
                            }
                            let count = self.vim_take_count();
                            let target = if count > 1 {
                                count.saturating_sub(1).min(total_lines - 1)
                            } else {
                                total_lines - 1
                            };
                            self.vim_move_cursor_to_line(target, true);
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('^'),
                            ..
                        } => {
                            let line_idx = self.vim_current_line_index();
                            self.vim_move_cursor_to_line(line_idx, true);
                            self.vim_clear_count();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('f'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            self.vim_pending_find = Some(VimPendingFind::ForwardInclusive);
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('t'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            self.vim_pending_find = Some(VimPendingFind::ForwardExclusive);
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char(';'),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            let Some(state) = self.vim_last_find else {
                                self.vim_clear_count();
                                return;
                            };
                            let count = self.vim_take_count();
                            if !self.vim_perform_find(
                                state.target,
                                state.direction,
                                state.stop_before,
                                count,
                            ) {
                                self.vim_clear_count();
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char(','),
                            modifiers: KeyModifiers::NONE,
                            ..
                        } => {
                            let Some(state) = self.vim_last_find else {
                                self.vim_clear_count();
                                return;
                            };
                            let direction = match state.direction {
                                VimFindDirection::Forward => VimFindDirection::Backward,
                                VimFindDirection::Backward => VimFindDirection::Forward,
                            };
                            let count = self.vim_take_count();
                            if !self.vim_perform_find(
                                state.target,
                                direction,
                                state.stop_before,
                                count,
                            ) {
                                self.vim_clear_count();
                            }
                            return;
                        }
                        // Enter Insert mode
                        KeyEvent {
                            code: KeyCode::Char('i'),
                            ..
                        } => {
                            if self.vim_pending_op.is_some() {
                                if self.vim_recording_command {
                                    self.push_vim_command_event(event);
                                }
                                self.vim_pending_textobj = Some(VimPendingTextObject::Inner);
                                return;
                            }
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('I'),
                            ..
                        } => {
                            self.move_cursor_to_beginning_of_line(false);
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('a'),
                            ..
                        } => {
                            if self.vim_pending_op.is_some() {
                                if self.vim_recording_command {
                                    self.push_vim_command_event(event);
                                }
                                self.vim_pending_textobj = Some(VimPendingTextObject::Around);
                                return;
                            }
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.move_cursor_right();
                            }
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('A'),
                            ..
                        } => {
                            self.move_cursor_to_end_of_line(false);
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('o'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            // newline below current line
                            let eol = self.end_of_current_line();
                            self.set_cursor(eol);
                            self.insert_str("\n");
                            self.vim_enter_insert_mode();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('O'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            // newline above current line
                            let bol = self.beginning_of_current_line();
                            self.set_cursor(bol);
                            self.insert_str("\n");
                            self.set_cursor(bol);
                            self.vim_enter_insert_mode();
                            return;
                        }

                        // Movement
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Left,
                            ..
                        } => {
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.move_cursor_left();
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('l'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Right,
                            ..
                        } => {
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.move_cursor_right();
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('k'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Up, ..
                        } => {
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.move_cursor_up();
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('j'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Down,
                            ..
                        } => {
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                self.move_cursor_down();
                            }
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('0'),
                            ..
                        } => {
                            if let Some(op) = self.vim_pending_op.take() {
                                let count = self.vim_take_count();
                                let start = self.beginning_of_current_line();
                                let mut target = self.cursor_pos;
                                for _ in 0..count.max(1) {
                                    target = self.prev_atomic_boundary(target);
                                    if target == start {
                                        break;
                                    }
                                }
                                let range_start = target.min(self.cursor_pos);
                                match op {
                                    VimPendingOperator::Delete => {
                                        self.vim_store_range(range_start, self.cursor_pos);
                                        self.replace_range(range_start..self.cursor_pos, "");
                                        self.set_cursor(range_start);
                                        self.vim_pending_op = None;
                                        self.vim_clear_count();
                                    }
                                    VimPendingOperator::Change => {
                                        self.vim_store_range(range_start, self.cursor_pos);
                                        self.replace_range(range_start..self.cursor_pos, "");
                                        self.set_cursor(range_start);
                                        self.vim_enter_insert_mode();
                                    }
                                    VimPendingOperator::Yank => {
                                        self.vim_store_range(range_start, self.cursor_pos);
                                        self.vim_clear_count();
                                        self.vim_pending_op = None;
                                        return;
                                    }
                                }
                                return;
                            }
                            self.move_cursor_to_beginning_of_line(false);
                            self.vim_clear_count();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('$'),
                            ..
                        } => {
                            if let Some(op) = self.vim_pending_op {
                                let count = self.vim_take_count();
                                match op {
                                    VimPendingOperator::Delete => {
                                        for _ in 0..count.max(1) {
                                            self.kill_to_end_of_line();
                                            if self.cursor_pos < self.text.len()
                                                && self.text.as_bytes()[self.cursor_pos] == b'\n'
                                            {
                                                self.delete_forward(1);
                                            }
                                        }
                                        self.vim_pending_op = None;
                                    }
                                    VimPendingOperator::Change => {
                                        let end = self.end_of_current_line();
                                        self.vim_change_forward_to(end);
                                    }
                                    VimPendingOperator::Yank => {
                                        self.vim_pending_op = Some(VimPendingOperator::Yank);
                                        self.vim_clear_count();
                                        return;
                                    }
                                }
                                return;
                            }
                            self.move_cursor_to_end_of_line(false);
                            self.vim_clear_count();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('b'),
                            ..
                        } => {
                            let count = self.vim_take_count();
                            for _ in 0..count {
                                let p = self.beginning_of_previous_word();
                                self.set_cursor(p);
                            }
                            return;
                        }
                        // Edits
                        KeyEvent {
                            code: KeyCode::Char('x'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            if self.vim_pending_op.is_some() {
                                self.vim_pending_op = None;
                            }
                            let count = self.vim_take_count();
                            let start = self.cursor_pos;
                            let mut end = start;
                            for _ in 0..count {
                                end = self.next_atomic_boundary(end);
                                if end >= self.text.len() {
                                    break;
                                }
                            }
                            self.vim_store_range(start, end);
                            self.delete_forward(count);
                            self.finish_vim_command();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('X'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            if self.vim_pending_op.is_some() {
                                self.vim_pending_op = None;
                            }
                            let count = self.vim_take_count();
                            let mut start = self.cursor_pos;
                            for _ in 0..count {
                                start = self.prev_atomic_boundary(start);
                                if start == 0 {
                                    break;
                                }
                            }
                            self.vim_store_range(start, self.cursor_pos);
                            self.delete_backward(count);
                            self.finish_vim_command();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('p'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            let count = self.vim_take_count();
                            self.vim_put_after(count);
                            self.finish_vim_command();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('P'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            let count = self.vim_take_count();
                            self.vim_put_before(count);
                            self.finish_vim_command();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char(_),
                            ..
                        } if self.vim_pending_op.is_some() => {
                            self.mark_unhandled_vim_key(event);
                            self.abort_vim_command();
                            self.vim_pending_op = None;
                            self.vim_pending_textobj = None;
                            self.vim_clear_count();
                            return;
                        }
                        KeyEvent {
                            code: KeyCode::Char('D'),
                            ..
                        } => {
                            if self.vim_recording_command {
                                self.abort_vim_command();
                            }
                            self.begin_vim_command(event);
                            let count = self.vim_take_count();
                            for i in 0..count {
                                self.kill_to_end_of_line();
                                if i + 1 < count
                                    && self.cursor_pos < self.text.len()
                                    && self.text.as_bytes()[self.cursor_pos] == b'\n'
                                {
                                    self.delete_forward(1);
                                }
                            }
                            self.finish_vim_command();
                            return;
                        }
                        _ => {
                            self.vim_clear_count();
                            // Ignore other keys in Normal mode
                            self.mark_unhandled_vim_key(event);
                            return;
                        }
                    }
                }
                VimState::Insert => {
                    // fall through to default behavior below
                }
            }
        }

        let recording_insert = self.vim_mode_enabled
            && matches!(self.vim_state, VimState::Insert)
            && self.vim_recording_command;
        if recording_insert {
            self.push_vim_command_event(event);
        }

        match event {
            // Some terminals (or configurations) send Control key chords as
            // C0 control characters without reporting the CONTROL modifier.
            // Handle common fallbacks for Ctrl-B/Ctrl-F here so they don't get
            // inserted as literal control bytes.
            KeyEvent { code: KeyCode::Char('\u{0002}'), modifiers: KeyModifiers::NONE, .. } /* ^B */ => {
                self.move_cursor_left();
            }
            KeyEvent { code: KeyCode::Char('\u{0006}'), modifiers: KeyModifiers::NONE, .. } /* ^F */ => {
                self.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                // Insert plain characters (and Shift-modified). Do NOT insert when ALT is held,
                // because many terminals map Option/Meta combos to ALT+<char> (e.g. ESC f/ESC b)
                // for word navigation. Those are handled explicitly below.
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => self.insert_str(&c.to_string()),
            KeyEvent {
                code: KeyCode::Char('j' | 'm'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.insert_str("\n"),
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers,
                ..
            } if modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.delete_backward_word()
            },
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::ALT,
                ..
            } => self.delete_backward_word(),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.delete_backward(1),
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::ALT,
                ..
            }  => self.delete_forward_word(),
            KeyEvent {
                code: KeyCode::Delete,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.delete_forward(1),

            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.delete_backward_word();
            }
            // Meta-b -> move to beginning of previous word
            // Meta-f -> move to end of next word
            // Many terminals map Option (macOS) to Alt. Some send Alt|Shift, so match contains(ALT).
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.set_cursor(self.beginning_of_previous_word());
            }
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.set_cursor(self.end_of_next_word());
            }
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.kill_to_beginning_of_line();
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.kill_to_end_of_line();
            }

            // Cursor movement
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_right();
            }
            // Some terminals send Alt+Arrow for word-wise movement:
            // Option/Left -> Alt+Left (previous word start)
            // Option/Right -> Alt+Right (next word end)
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::ALT,
                ..
            }
            | KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.set_cursor(self.beginning_of_previous_word());
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::ALT,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.set_cursor(self.end_of_next_word());
            }
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                self.move_cursor_down();
            }
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.move_cursor_to_beginning_of_line(false);
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_to_beginning_of_line(true);
            }

            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.move_cursor_to_end_of_line(false);
            }
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_to_end_of_line(true);
            }
            _o => {
                #[cfg(feature = "debug-logs")]
                tracing::debug!("Unhandled key event in TextArea: {:?}", _o);
            }
        }
    }

    /// Enable or disable Vim key bindings. When enabled, the default state is Insert.
    pub fn set_vim_mode_enabled(&mut self, enabled: bool) {
        self.vim_mode_enabled = enabled;
        self.vim_state = VimState::Insert;
        self.vim_pending_op = None;
        self.vim_input_count = None;
        self.vim_last_change = None;
        self.vim_pending_find = None;
        self.vim_pending_g = false;
        self.vim_last_find = None;
        self.vim_last_unhandled = None;
    }

    /// Returns true when Vim mode is enabled and the state machine is currently in Normal mode.
    pub fn vim_is_normal_mode(&self) -> bool {
        self.vim_mode_enabled && matches!(self.vim_state, VimState::Normal)
    }

    /// Return a short label for the current Vim mode, if Vim is enabled.
    /// Used by the footer to display INSERT/NORMAL.
    pub fn vim_mode_state_label(&self) -> Option<String> {
        if !self.vim_mode_enabled {
            return None;
        }
        if matches!(self.vim_state, VimState::Insert) {
            return Some("INSERT".to_string());
        }

        let mut parts: Vec<String> = vec!["NORMAL".to_string()];

        if let Some(count) = self.vim_input_count {
            parts.push(format!("count:{count}"));
        }

        if let Some(op) = self.vim_pending_op {
            let label = match op {
                VimPendingOperator::Delete => "d…",
                VimPendingOperator::Change => "c…",
                VimPendingOperator::Yank => "y…",
            };
            parts.push(label.to_string());
        }

        if self.vim_pending_g {
            parts.push("g?".to_string());
        }

        if let Some(obj) = self.vim_pending_textobj {
            let label = match obj {
                VimPendingTextObject::Inner => "iw?",
                VimPendingTextObject::Around => "aw?",
            };
            parts.push(label.to_string());
        }

        if let Some(find) = self.vim_pending_find {
            let symbol = match find {
                VimPendingFind::ForwardInclusive => "f",
                VimPendingFind::ForwardExclusive => "t",
            };
            parts.push(format!("{symbol}?"));
        }

        if self.vim_pending_replace {
            parts.push("r?".to_string());
        }

        if !self.vim_last_command.is_empty() && !self.vim_recording_command {
            parts.push(".ready".to_string());
        }

        if parts.len() == 1 {
            return Some(parts.into_iter().next().unwrap());
        }

        Some(parts.join(" · "))
    }

    pub fn vim_mode_enabled(&self) -> bool {
        self.vim_mode_enabled
    }

    fn vim_delete_current_line(&mut self) {
        let bol = self.beginning_of_current_line();
        let eol = self.end_of_current_line();
        // Delete newline after line if present to remove the line cleanly.
        let mut end = eol;
        if eol < self.text.len() {
            // There is a newline following this line.
            end = end.saturating_add(1);
        }
        self.vim_store_range(bol, end);
        self.replace_range(bol..end, "");
        // Cursor is placed at start of the deleted region by replace_range
        // which matches expected behavior for dd.
    }

    fn vim_delete_forward_to(&mut self, mut end: usize) {
        let start = self.cursor_pos;
        if end == start {
            // Ensure we delete at least one atom if possible.
            end = self.next_atomic_boundary(start);
        }
        if end > start {
            self.vim_store_range(start, end);
            self.replace_range(start..end, "");
        }
    }

    // ####### Input Functions #######
    pub fn delete_backward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos == 0 {
            return;
        }
        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.prev_atomic_boundary(target);
            if target == 0 {
                break;
            }
        }
        self.replace_range(target..self.cursor_pos, "");
    }

    pub fn delete_forward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos >= self.text.len() {
            return;
        }
        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.next_atomic_boundary(target);
            if target >= self.text.len() {
                break;
            }
        }
        self.replace_range(self.cursor_pos..target, "");
    }

    pub fn delete_backward_word(&mut self) {
        let start = self.beginning_of_previous_word();
        self.replace_range(start..self.cursor_pos, "");
    }

    /// Delete text to the right of the cursor using "word" semantics.
    ///
    /// Deletes from the current cursor position through the end of the next word as determined
    /// by `end_of_next_word()`. Any whitespace (including newlines) between the cursor and that
    /// word is included in the deletion.
    pub fn delete_forward_word(&mut self) {
        let end = self.end_of_next_word();
        if end > self.cursor_pos {
            self.replace_range(self.cursor_pos..end, "");
        }
    }

    pub fn kill_to_end_of_line(&mut self) {
        let eol = self.end_of_current_line();
        if self.cursor_pos == eol {
            if eol < self.text.len() {
                self.vim_store_range(self.cursor_pos, eol + 1);
                self.replace_range(self.cursor_pos..eol + 1, "");
            }
        } else {
            self.vim_store_range(self.cursor_pos, eol);
            self.replace_range(self.cursor_pos..eol, "");
        }
    }

    pub fn kill_to_beginning_of_line(&mut self) {
        let bol = self.beginning_of_current_line();
        if self.cursor_pos == bol {
            if bol > 0 {
                self.vim_store_range(bol - 1, bol);
                self.replace_range(bol - 1..bol, "");
            }
        } else {
            self.vim_store_range(bol, self.cursor_pos);
            self.replace_range(bol..self.cursor_pos, "");
        }
    }

    /// Move the cursor left by a single grapheme cluster.
    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.prev_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    /// Move the cursor right by a single grapheme cluster.
    pub fn move_cursor_right(&mut self) {
        self.cursor_pos = self.next_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    pub fn move_cursor_up(&mut self) {
        // If we have a wrapping cache, prefer navigating across wrapped (visual) lines.
        if let Some((target_col, maybe_line)) = {
            let cache_ref = self.wrap_cache.borrow();
            if let Some(cache) = cache_ref.as_ref() {
                let lines = &cache.lines;
                if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                    let cur_range = &lines[idx];
                    let target_col = self
                        .preferred_col
                        .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());
                    if idx > 0 {
                        let prev = &lines[idx - 1];
                        let line_start = prev.start;
                        let line_end = prev.end.saturating_sub(1);
                        Some((target_col, Some((line_start, line_end))))
                    } else {
                        Some((target_col, None))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } {
            // We had wrapping info. Apply movement accordingly.
            match maybe_line {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    // Already at first visual line -> move to start
                    self.cursor_pos = 0;
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation if we don't have wrapping info yet.
        if let Some(prev_nl) = self.text[..self.cursor_pos].rfind('\n') {
            let target_col = match self.preferred_col {
                Some(c) => c,
                None => {
                    let c = self.current_display_col();
                    self.preferred_col = Some(c);
                    c
                }
            };
            let prev_line_start = self.text[..prev_nl].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let prev_line_end = prev_nl;
            self.move_to_display_col_on_line(prev_line_start, prev_line_end, target_col);
        } else {
            self.cursor_pos = 0;
            self.preferred_col = None;
        }
    }

    pub fn move_cursor_down(&mut self) {
        // If we have a wrapping cache, prefer navigating across wrapped (visual) lines.
        if let Some((target_col, move_to_last)) = {
            let cache_ref = self.wrap_cache.borrow();
            if let Some(cache) = cache_ref.as_ref() {
                let lines = &cache.lines;
                if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                    let cur_range = &lines[idx];
                    let target_col = self
                        .preferred_col
                        .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());
                    if idx + 1 < lines.len() {
                        let next = &lines[idx + 1];
                        let line_start = next.start;
                        let line_end = next.end.saturating_sub(1);
                        Some((target_col, Some((line_start, line_end))))
                    } else {
                        Some((target_col, None))
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } {
            match move_to_last {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    // Already on last visual line -> move to end
                    self.cursor_pos = self.text.len();
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation if we don't have wrapping info yet.
        let target_col = match self.preferred_col {
            Some(c) => c,
            None => {
                let c = self.current_display_col();
                self.preferred_col = Some(c);
                c
            }
        };
        if let Some(next_nl) = self.text[self.cursor_pos..]
            .find('\n')
            .map(|i| i + self.cursor_pos)
        {
            let next_line_start = next_nl + 1;
            let next_line_end = self.text[next_line_start..]
                .find('\n')
                .map(|i| i + next_line_start)
                .unwrap_or(self.text.len());
            self.move_to_display_col_on_line(next_line_start, next_line_end, target_col);
        } else {
            self.cursor_pos = self.text.len();
            self.preferred_col = None;
        }
    }

    pub fn move_cursor_to_beginning_of_line(&mut self, move_up_at_bol: bool) {
        let bol = self.beginning_of_current_line();
        if move_up_at_bol && self.cursor_pos == bol {
            self.set_cursor(self.beginning_of_line(self.cursor_pos.saturating_sub(1)));
        } else {
            self.set_cursor(bol);
        }
        self.preferred_col = None;
    }

    pub fn move_cursor_to_end_of_line(&mut self, move_down_at_eol: bool) {
        let eol = self.end_of_current_line();
        if move_down_at_eol && self.cursor_pos == eol {
            let next_pos = (self.cursor_pos.saturating_add(1)).min(self.text.len());
            self.set_cursor(self.end_of_line(next_pos));
        } else {
            self.set_cursor(eol);
        }
    }

    // ===== Text elements support =====

    pub fn insert_element(&mut self, text: &str) {
        let start = self.clamp_pos_for_insertion(self.cursor_pos);
        self.insert_str_at(start, text);
        let end = start + text.len();
        self.add_element(start..end);
        // Place cursor at end of inserted element
        self.set_cursor(end);
    }

    fn add_element(&mut self, range: Range<usize>) {
        let elem = TextElement { range };
        self.elements.push(elem);
        self.elements.sort_by_key(|e| e.range.start);
    }

    fn find_element_containing(&self, pos: usize) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| pos > e.range.start && pos < e.range.end)
    }

    fn clamp_pos_to_nearest_boundary(&self, mut pos: usize) -> usize {
        if pos > self.text.len() {
            pos = self.text.len();
        }
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);
            if dist_start <= dist_end {
                e.range.start
            } else {
                e.range.end
            }
        } else {
            pos
        }
    }

    fn clamp_pos_for_insertion(&self, pos: usize) -> usize {
        // Do not allow inserting into the middle of an element
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            // Choose closest edge for insertion
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);
            if dist_start <= dist_end {
                e.range.start
            } else {
                e.range.end
            }
        } else {
            pos
        }
    }

    fn expand_range_to_element_boundaries(&self, mut range: Range<usize>) -> Range<usize> {
        // Expand to include any intersecting elements fully
        loop {
            let mut changed = false;
            for e in &self.elements {
                if e.range.start < range.end && e.range.end > range.start {
                    let new_start = range.start.min(e.range.start);
                    let new_end = range.end.max(e.range.end);
                    if new_start != range.start || new_end != range.end {
                        range.start = new_start;
                        range.end = new_end;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        range
    }

    fn shift_elements(&mut self, at: usize, removed: usize, inserted: usize) {
        // Generic shift: for pure insert, removed = 0; for delete, inserted = 0.
        let end = at + removed;
        let diff = inserted as isize - removed as isize;
        // Remove elements fully deleted by the operation and shift the rest
        self.elements
            .retain(|e| !(e.range.start >= at && e.range.end <= end));
        for e in &mut self.elements {
            if e.range.end <= at {
                // before edit
            } else if e.range.start >= end {
                // after edit
                e.range.start = ((e.range.start as isize) + diff) as usize;
                e.range.end = ((e.range.end as isize) + diff) as usize;
            } else {
                // Overlap with element but not fully contained (shouldn't happen when using
                // element-aware replace, but degrade gracefully by snapping element to new bounds)
                let new_start = at.min(e.range.start);
                let new_end = at + inserted.max(e.range.end.saturating_sub(end));
                e.range.start = new_start;
                e.range.end = new_end;
            }
        }
    }

    fn update_elements_after_replace(&mut self, start: usize, end: usize, inserted_len: usize) {
        self.shift_elements(start, end.saturating_sub(start), inserted_len);
    }

    fn prev_atomic_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        // If currently at an element end or inside, jump to start of that element.
        if let Some(idx) = self
            .elements
            .iter()
            .position(|e| pos > e.range.start && pos <= e.range.end)
        {
            return self.elements[idx].range.start;
        }
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.prev_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.start
                } else {
                    b
                }
            }
            Ok(None) => 0,
            Err(_) => pos.saturating_sub(1),
        }
    }

    fn next_atomic_boundary(&self, pos: usize) -> usize {
        if pos >= self.text.len() {
            return self.text.len();
        }
        // If currently at an element start or inside, jump to end of that element.
        if let Some(idx) = self
            .elements
            .iter()
            .position(|e| pos >= e.range.start && pos < e.range.end)
        {
            return self.elements[idx].range.end;
        }
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.next_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.end
                } else {
                    b
                }
            }
            Ok(None) => self.text.len(),
            Err(_) => pos.saturating_add(1),
        }
    }

    pub(crate) fn beginning_of_previous_word(&self) -> usize {
        if let Some(first_non_ws) = self.text[..self.cursor_pos].rfind(|c: char| !c.is_whitespace())
        {
            let candidate = self.text[..first_non_ws]
                .rfind(|c: char| c.is_whitespace())
                .map(|i| i + 1)
                .unwrap_or(0);
            self.adjust_pos_out_of_elements(candidate, true)
        } else {
            0
        }
    }

    pub(crate) fn end_of_next_word(&self) -> usize {
        let Some(first_non_ws) = self.text[self.cursor_pos..].find(|c: char| !c.is_whitespace())
        else {
            return self.text.len();
        };
        let word_start = self.cursor_pos + first_non_ws;
        let candidate = match self.text[word_start..].find(|c: char| c.is_whitespace()) {
            Some(rel_idx) => word_start + rel_idx,
            None => self.text.len(),
        };
        self.adjust_pos_out_of_elements(candidate, false)
    }

    /// Start of the next word (Vim 'w'). If currently on a word, moves to the
    /// start of the following word; if on whitespace, moves to the first
    /// non-whitespace ahead. Falls back to end-of-text when no next word.
    pub(crate) fn beginning_of_next_word(&self) -> usize {
        let text = &self.text[self.cursor_pos..];
        if text.is_empty() {
            return self.text.len();
        }
        let mut offset = 0usize;
        let mut chars = text.chars();
        let first = chars.next().unwrap();
        if first.is_whitespace() {
            // Skip whitespace to the start of the next word.
            offset += 1;
            for c in chars {
                if !c.is_whitespace() {
                    break;
                }
                offset += 1;
            }
            let pos = (self.cursor_pos + offset).min(self.text.len());
            self.adjust_pos_out_of_elements(pos, true)
        } else {
            // Inside a word: skip to its end, then skip following whitespace to next word start.
            // Find end of current word
            let rest = &self.text[self.cursor_pos..];
            let end_rel = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
            let after_word = self.cursor_pos + end_rel;
            let after = &self.text[after_word..];
            if after.is_empty() {
                return self.text.len();
            }
            let ws = after.find(|c: char| !c.is_whitespace());
            let pos = match ws {
                Some(non_ws) => after_word + non_ws,
                None => self.text.len(),
            };
            self.adjust_pos_out_of_elements(pos, true)
        }
    }

    fn adjust_pos_out_of_elements(&self, pos: usize, prefer_start: bool) -> usize {
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            if prefer_start {
                e.range.start
            } else {
                e.range.end
            }
        } else {
            pos
        }
    }

    #[expect(clippy::unwrap_used)]
    fn wrapped_lines(&self, width: u16) -> Ref<'_, Vec<Range<usize>>> {
        // Ensure cache is ready (potentially mutably borrow, then drop)
        {
            let mut cache = self.wrap_cache.borrow_mut();
            let needs_recalc = match cache.as_ref() {
                Some(c) => c.width != width,
                None => true,
            };
            if needs_recalc {
                let lines = crate::wrapping::wrap_ranges(
                    &self.text,
                    Options::new(width as usize).wrap_algorithm(textwrap::WrapAlgorithm::FirstFit),
                );
                *cache = Some(WrapCache { width, lines });
            }
        }

        let cache = self.wrap_cache.borrow();
        Ref::map(cache, |c| &c.as_ref().unwrap().lines)
    }

    /// Calculate the scroll offset that should be used to satisfy the
    /// invariants given the current area size and wrapped lines.
    ///
    /// - Cursor is always on screen.
    /// - No scrolling if content fits in the area.
    fn effective_scroll(
        &self,
        area_height: u16,
        lines: &[Range<usize>],
        current_scroll: u16,
    ) -> u16 {
        let total_lines = lines.len() as u16;
        if area_height >= total_lines {
            return 0;
        }

        // Where is the cursor within wrapped lines? Prefer assigning boundary positions
        // (where pos equals the start of a wrapped line) to that later line.
        let cursor_line_idx =
            Self::wrapped_line_index_by_start(lines, self.cursor_pos).unwrap_or(0) as u16;

        let max_scroll = total_lines.saturating_sub(area_height);
        let mut scroll = current_scroll.min(max_scroll);

        // Ensure cursor is visible within [scroll, scroll + area_height)
        if cursor_line_idx < scroll {
            scroll = cursor_line_idx;
        } else if cursor_line_idx >= scroll + area_height {
            scroll = cursor_line_idx + 1 - area_height;
        }
        scroll
    }
}

impl WidgetRef for &TextArea {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.wrapped_lines(area.width);
        self.render_lines(area, buf, &lines, 0..lines.len());
    }
}

impl StatefulWidgetRef for &TextArea {
    type State = TextAreaState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let lines = self.wrapped_lines(area.width);
        let scroll = self.effective_scroll(area.height, &lines, state.scroll);
        state.scroll = scroll;

        let start = scroll as usize;
        let end = (scroll + area.height).min(lines.len() as u16) as usize;
        self.render_lines(area, buf, &lines, start..end);
    }
}

impl TextArea {
    fn render_lines(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[Range<usize>],
        range: std::ops::Range<usize>,
    ) {
        for (row, idx) in range.enumerate() {
            let r = &lines[idx];
            let y = area.y + row as u16;
            let line_range = r.start..r.end - 1;
            // Draw base line with default style.
            buf.set_string(area.x, y, &self.text[line_range.clone()], Style::default());

            // Overlay styled segments for elements that intersect this line.
            for elem in &self.elements {
                // Compute overlap with displayed slice.
                let overlap_start = elem.range.start.max(line_range.start);
                let overlap_end = elem.range.end.min(line_range.end);
                if overlap_start >= overlap_end {
                    continue;
                }
                let styled = &self.text[overlap_start..overlap_end];
                let x_off = self.text[line_range.start..overlap_start].width() as u16;
                let style = Style::default().fg(Color::Cyan);
                buf.set_string(area.x + x_off, y, styled, style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // crossterm types are intentionally not imported here to avoid unused warnings
    use rand::prelude::*;

    fn rand_grapheme(rng: &mut rand::rngs::StdRng) -> String {
        let r: u8 = rng.random_range(0..100);
        match r {
            0..=4 => "\n".to_string(),
            5..=12 => " ".to_string(),
            13..=35 => (rng.random_range(b'a'..=b'z') as char).to_string(),
            36..=45 => (rng.random_range(b'A'..=b'Z') as char).to_string(),
            46..=52 => (rng.random_range(b'0'..=b'9') as char).to_string(),
            53..=65 => {
                // Some emoji (wide graphemes)
                let choices = ["👍", "😊", "🐍", "🚀", "🧪", "🌟"];
                choices[rng.random_range(0..choices.len())].to_string()
            }
            66..=75 => {
                // CJK wide characters
                let choices = ["漢", "字", "測", "試", "你", "好", "界", "编", "码"];
                choices[rng.random_range(0..choices.len())].to_string()
            }
            76..=85 => {
                // Combining mark sequences
                let base = ["e", "a", "o", "n", "u"][rng.random_range(0..5)];
                let marks = ["\u{0301}", "\u{0308}", "\u{0302}", "\u{0303}"];
                format!("{base}{}", marks[rng.random_range(0..marks.len())])
            }
            86..=92 => {
                // Some non-latin single codepoints (Greek, Cyrillic, Hebrew)
                let choices = ["Ω", "β", "Ж", "ю", "ש", "م", "ह"];
                choices[rng.random_range(0..choices.len())].to_string()
            }
            _ => {
                // ZWJ sequences (single graphemes but multi-codepoint)
                let choices = [
                    "👩\u{200D}💻", // woman technologist
                    "👨\u{200D}💻", // man technologist
                    "🏳️\u{200D}🌈", // rainbow flag
                ];
                choices[rng.random_range(0..choices.len())].to_string()
            }
        }
    }

    fn ta_with(text: &str) -> TextArea {
        let mut t = TextArea::new();
        t.insert_str(text);
        t.undo_stack.clear();
        t.redo_stack.clear();
        t
    }

    #[test]
    fn insert_and_replace_update_cursor_and_text() {
        // insert helpers
        let mut t = ta_with("hello");
        t.set_cursor(5);
        t.insert_str("!");
        assert_eq!(t.text(), "hello!");
        assert_eq!(t.cursor(), 6);

        t.insert_str_at(0, "X");
        assert_eq!(t.text(), "Xhello!");
        assert_eq!(t.cursor(), 7);

        // Insert after the cursor should not move it
        t.set_cursor(1);
        let end = t.text().len();
        t.insert_str_at(end, "Y");
        assert_eq!(t.text(), "Xhello!Y");
        assert_eq!(t.cursor(), 1);

        // replace_range cases
        // 1) cursor before range
        let mut t = ta_with("abcd");
        t.set_cursor(1);
        t.replace_range(2..3, "Z");
        assert_eq!(t.text(), "abZd");
        assert_eq!(t.cursor(), 1);

        // 2) cursor inside range
        let mut t = ta_with("abcd");
        t.set_cursor(2);
        t.replace_range(1..3, "Q");
        assert_eq!(t.text(), "aQd");
        assert_eq!(t.cursor(), 2);

        // 3) cursor after range with shifted by diff
        let mut t = ta_with("abcd");
        t.set_cursor(4);
        t.replace_range(0..1, "AA");
        assert_eq!(t.text(), "AAbcd");
        assert_eq!(t.cursor(), 5);
    }

    #[test]
    fn delete_backward_and_forward_edges() {
        let mut t = ta_with("abc");
        t.set_cursor(1);
        t.delete_backward(1);
        assert_eq!(t.text(), "bc");
        assert_eq!(t.cursor(), 0);

        // deleting backward at start is a no-op
        t.set_cursor(0);
        t.delete_backward(1);
        assert_eq!(t.text(), "bc");
        assert_eq!(t.cursor(), 0);

        // forward delete removes next grapheme
        t.set_cursor(1);
        t.delete_forward(1);
        assert_eq!(t.text(), "b");
        assert_eq!(t.cursor(), 1);

        // forward delete at end is a no-op
        t.set_cursor(t.text().len());
        t.delete_forward(1);
        assert_eq!(t.text(), "b");
    }

    #[test]
    fn delete_backward_word_and_kill_line_variants() {
        // delete backward word at end removes the whole previous word
        let mut t = ta_with("hello   world  ");
        t.set_cursor(t.text().len());
        t.delete_backward_word();
        assert_eq!(t.text(), "hello   ");
        assert_eq!(t.cursor(), 8);

        // From inside a word, delete from word start to cursor
        let mut t = ta_with("foo bar");
        t.set_cursor(6); // inside "bar" (after 'a')
        t.delete_backward_word();
        assert_eq!(t.text(), "foo r");
        assert_eq!(t.cursor(), 4);

        // From end, delete the last word only
        let mut t = ta_with("foo bar");
        t.set_cursor(t.text().len());
        t.delete_backward_word();
        assert_eq!(t.text(), "foo ");
        assert_eq!(t.cursor(), 4);

        // kill_to_end_of_line when not at EOL
        let mut t = ta_with("abc\ndef");
        t.set_cursor(1); // on first line, middle
        t.kill_to_end_of_line();
        assert_eq!(t.text(), "a\ndef");
        assert_eq!(t.cursor(), 1);

        // kill_to_end_of_line when at EOL deletes newline
        let mut t = ta_with("abc\ndef");
        t.set_cursor(3); // EOL of first line
        t.kill_to_end_of_line();
        assert_eq!(t.text(), "abcdef");
        assert_eq!(t.cursor(), 3);

        // kill_to_beginning_of_line from middle of line
        let mut t = ta_with("abc\ndef");
        t.set_cursor(5); // on second line, after 'e'
        t.kill_to_beginning_of_line();
        assert_eq!(t.text(), "abc\nef");

        // kill_to_beginning_of_line at beginning of non-first line removes the previous newline
        let mut t = ta_with("abc\ndef");
        t.set_cursor(4); // beginning of second line
        t.kill_to_beginning_of_line();
        assert_eq!(t.text(), "abcdef");
        assert_eq!(t.cursor(), 3);
    }

    #[test]
    fn delete_forward_word_variants() {
        let mut t = ta_with("hello   world ");
        t.set_cursor(0);
        t.delete_forward_word();
        assert_eq!(t.text(), "   world ");
        assert_eq!(t.cursor(), 0);

        let mut t = ta_with("hello   world ");
        t.set_cursor(1);
        t.delete_forward_word();
        assert_eq!(t.text(), "h   world ");
        assert_eq!(t.cursor(), 1);

        let mut t = ta_with("hello   world");
        t.set_cursor(t.text().len());
        t.delete_forward_word();
        assert_eq!(t.text(), "hello   world");
        assert_eq!(t.cursor(), t.text().len());

        let mut t = ta_with("foo   \nbar");
        t.set_cursor(3);
        t.delete_forward_word();
        assert_eq!(t.text(), "foo");
        assert_eq!(t.cursor(), 3);

        let mut t = ta_with("foo\nbar");
        t.set_cursor(3);
        t.delete_forward_word();
        assert_eq!(t.text(), "foo");
        assert_eq!(t.cursor(), 3);

        let mut t = ta_with("hello   world ");
        t.set_cursor(t.text().len() + 10);
        t.delete_forward_word();
        assert_eq!(t.text(), "hello   world ");
        assert_eq!(t.cursor(), t.text().len());
    }

    #[test]
    fn delete_forward_word_handles_atomic_elements() {
        let mut t = TextArea::new();
        t.insert_element("<element>");
        t.insert_str(" tail");

        t.set_cursor(0);
        t.delete_forward_word();
        assert_eq!(t.text(), " tail");
        assert_eq!(t.cursor(), 0);

        let mut t = TextArea::new();
        t.insert_str("   ");
        t.insert_element("<element>");
        t.insert_str(" tail");

        t.set_cursor(0);
        t.delete_forward_word();
        assert_eq!(t.text(), " tail");
        assert_eq!(t.cursor(), 0);

        let mut t = TextArea::new();
        t.insert_str("prefix ");
        t.insert_element("<element>");
        t.insert_str(" tail");

        // cursor in the middle of the element, delete_forward_word deletes the element
        let elem_range = t.elements[0].range.clone();
        t.cursor_pos = elem_range.start + (elem_range.len() / 2);
        t.delete_forward_word();
        assert_eq!(t.text(), "prefix  tail");
        assert_eq!(t.cursor(), elem_range.start);
    }

    #[test]
    fn cursor_left_and_right_handle_graphemes() {
        let mut t = ta_with("a👍b");
        t.set_cursor(t.text().len());

        t.move_cursor_left(); // before 'b'
        let after_first_left = t.cursor();
        t.move_cursor_left(); // before '👍'
        let after_second_left = t.cursor();
        t.move_cursor_left(); // before 'a'
        let after_third_left = t.cursor();

        assert!(after_first_left < t.text().len());
        assert!(after_second_left < after_first_left);
        assert!(after_third_left < after_second_left);

        // Move right back to end safely
        t.move_cursor_right();
        t.move_cursor_right();
        t.move_cursor_right();
        assert_eq!(t.cursor(), t.text().len());
    }

    #[test]
    fn control_b_and_f_move_cursor() {
        let mut t = ta_with("abcd");
        t.set_cursor(1);

        t.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert_eq!(t.cursor(), 2);

        t.input(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        assert_eq!(t.cursor(), 1);
    }

    #[test]
    fn control_b_f_fallback_control_chars_move_cursor() {
        let mut t = ta_with("abcd");
        t.set_cursor(2);

        // Simulate terminals that send C0 control chars without CONTROL modifier.
        // ^B (U+0002) should move left
        t.input(KeyEvent::new(KeyCode::Char('\u{0002}'), KeyModifiers::NONE));
        assert_eq!(t.cursor(), 1);

        // ^F (U+0006) should move right
        t.input(KeyEvent::new(KeyCode::Char('\u{0006}'), KeyModifiers::NONE));
        assert_eq!(t.cursor(), 2);
    }

    #[test]
    fn vim_mode_basic_navigation_and_insert() {
        let mut t = ta_with("hello world\nsecond");
        // Enable Vim mode (defaults to Insert state)
        t.set_vim_mode_enabled(true);

        // ESC -> Normal
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        // Place cursor at BOL to ensure 'l' can move right
        t.set_cursor(0);
        // 'l' moves right from BOL
        let c0 = t.cursor();
        t.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert!(t.cursor() > c0);
        // '0' -> BOL
        t.input(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        assert_eq!(t.cursor(), 0);
        // '$' -> EOL of first line
        t.input(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
        let eol = "hello world".len();
        assert_eq!(t.cursor(), eol);
        // 'k' at first line should stay within bounds (no panic)
        t.input(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));

        // 'b' beginning of previous word from end of first line
        t.input(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        assert!(t.cursor() < eol);
        // 'e' end of next word
        t.input(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert!(t.cursor() <= eol);

        // 'i' enter insert mode and insert a char
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        let before = t.text().len();
        t.input(KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::NONE));
        assert_eq!(t.text().len(), before + 1);

        // ESC normal; 'x' deletes one char
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let before_del = t.text().len();
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(t.text().len(), before_del.saturating_sub(1));
    }

    #[test]
    fn vim_mode_dd_deletes_current_line() {
        let mut t = ta_with("alpha\nbeta\ngamma");
        // Position cursor somewhere on the middle line
        let pos_beta = t.text().find("beta").unwrap() + 2; // inside 'beta'
        t.set_cursor(pos_beta);
        t.set_vim_mode_enabled(true);
        // Enter Normal
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // dd
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        // Expect 'beta\n' removed; remaining text should be "alpha\ngamma" or "alpha\n\ngamma" depending on trailing newline handling.
        assert_eq!(t.text(), "alpha\ngamma");
        // Cursor should be at the position where the deleted line used to start
        // i.e., at the newline between 'alpha' and 'gamma', or at start of 'gamma'
        let pos_gamma = t.text().find("gamma").unwrap();
        assert!(t.cursor() == pos_gamma || t.cursor() == pos_gamma.saturating_sub(1));
    }

    #[test]
    fn vim_mode_dw_and_de() {
        // Case 1: 'dw' from start of word removes word and trailing space
        let mut t = ta_with("foo bar baz");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        // dw
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(t.text(), "bar baz");
        assert_eq!(t.cursor(), 0);

        // Case 2: 'de' from start of word removes word but keeps one space
        let mut t = ta_with("foo bar");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert_eq!(t.text(), " bar");
        assert_eq!(t.cursor(), 0);

        // Case 3: 'dw' on whitespace deletes up to next word start (removes all spaces)
        let mut t = ta_with("foo   bar");
        // put cursor on one of the spaces
        let pos = "foo".len();
        t.set_cursor(pos);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(t.text(), "foobar");
        assert_eq!(t.cursor(), pos);
    }

    #[test]
    fn vim_mode_counted_motions_and_deletes() {
        let mut t = ta_with("abcdef");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.set_cursor(0);

        // 3l moves three characters to the right
        t.input(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(t.cursor(), 3);

        // 2x deletes two characters forward
        t.input(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(t.text(), "abcf");
        assert_eq!(t.cursor(), 3);
    }

    #[test]
    fn vim_mode_gg_and_g_respect_counts() {
        let mut t = ta_with("alpha\nbeta\n  gamma");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // Plain G -> last line, first non-blank character
        t.input(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'g'));

        // 2G -> second line start (first non-blank 'b')
        t.input(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'b'));

        // 3gg -> third line first non-blank 'g'
        t.input(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'g'));

        // plain gg -> first line first non-blank 'a'
        t.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'a'));
    }

    #[test]
    fn vim_mode_find_and_repeat() {
        let mut t = ta_with("axbxcdx");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.set_cursor(0);

        // Move to first 'x' to the right
        t.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'x'));

        // ';' repeat should move to next 'x'
        t.input(KeyEvent::new(KeyCode::Char(';'), KeyModifiers::NONE));
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'x'));
        // now cursor should be at index 3 (after 'b')
        assert_eq!(t.cursor(), 3);

        // ',' should move back to previous 'x'
        t.input(KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE));
        assert_eq!(t.cursor(), 1);
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'x'));

        // 't' should land before next 'x'
        t.input(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        // Should now be on 'b' (the character before the next 'x')
        assert_eq!(t.text().as_bytes().get(t.cursor()), Some(&b'b'));
    }

    #[test]
    fn vim_mode_yy_put_paste_linewise() {
        let mut t = ta_with("foo\nbar\n");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.set_cursor(0);

        // Yank first line
        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

        // Move to line 2 and paste below
        t.input(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo\nbar\nfoo\n");
    }

    #[test]
    fn vim_mode_yw_and_put() {
        let mut t = ta_with("hello world");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.set_cursor(0);

        // Yank the first word (including trailing space)
        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        // Move to end and paste after
        t.input(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        assert_eq!(t.text(), "hello worldhello ");
    }

    #[test]
    fn vim_mode_ciw_deletes_word_and_enters_insert() {
        let mut t = ta_with("foo bar");
        let pos = t.text().find("bar").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo ");
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("INSERT"));
        assert_eq!(t.cursor(), 4);
    }

    #[test]
    fn vim_mode_diw_removes_word() {
        let mut t = ta_with("foo bar baz");
        let pos = t.text().find("bar").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo  baz");
        assert_eq!(t.cursor(), pos);
    }

    #[test]
    fn vim_mode_yiw_and_put() {
        let mut t = ta_with("foo bar");
        let pos = t.text().find("bar").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo barbar");
    }

    #[test]
    fn vim_mode_caw_includes_trailing_space() {
        let mut t = ta_with("foo bar   baz");
        let pos = t.text().find("bar").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo baz");
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("INSERT"));
        assert_eq!(t.cursor(), pos);
    }

    #[test]
    fn vim_mode_yaw_captures_space() {
        let mut t = ta_with("foo bar baz");
        let pos = t.text().find("bar").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        // Move to end and paste after
        t.input(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

        assert_eq!(t.text(), "foo bar bazbar ");
    }

    #[test]
    fn vim_status_shows_dynamic_indicators() {
        let mut t = ta_with("hello world");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // No pending state: should report NORMAL.
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("NORMAL"));

        // Enter a count.
        t.input(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        let label = t.vim_mode_state_label().unwrap();
        assert!(
            label.contains("count:12"),
            "expected count label, got {label}"
        );

        // Await find target.
        t.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        let label = t.vim_mode_state_label().unwrap();
        assert!(
            label.contains("f?"),
            "expected find pending label, got {label}"
        );

        // Provide the target to move and clear pending find/count.
        t.input(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));

        // Perform an edit to record a repeatable command.
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        let label = t.vim_mode_state_label().unwrap();
        assert!(
            label.contains(".ready"),
            "expected dot-ready indicator, got {label}"
        );
    }

    #[test]
    fn vim_mode_phase_b_regressions() {
        let mut t = ta_with("foo bar baz");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);

        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.text(), "x bar baz");

        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "x x baz");

        t.set_text("hello brave world");
        let pos = "hello ".len() + 2;
        t.set_cursor(pos);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hello  world");
    }

    #[test]
    fn vim_mode_r_replaces_characters() {
        let mut t = ta_with("hello");
        t.set_cursor(1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // single replacement
        t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hAllo");
        assert_eq!(t.cursor(), 1);

        // count-based replacement
        t.input(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hxxlo");
        assert_eq!(t.cursor(), 2);
    }

    #[test]
    fn vim_mode_dot_repeats_dw() {
        let mut t = ta_with("one two three");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(t.text(), "two three");

        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "three");
    }

    #[test]
    fn vim_mode_dot_repeats_ciw_with_insert() {
        let mut t = ta_with("foo bar baz");
        let pos = t.text().find("foo").unwrap();
        t.set_cursor(pos + 1);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.text(), "X bar baz");

        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "X X baz");
    }

    #[test]
    fn vim_mode_dot_repeats_r_command() {
        let mut t = ta_with("abcd");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
        assert_eq!(t.text(), "aXcd");

        t.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "aXXd");
    }

    #[test]
    fn vim_mode_dot_repeats_paste() {
        let mut t = ta_with("hello world");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hello worldhello ");

        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hello worldhello hello ");
    }

    #[test]
    fn vim_mode_dot_repeats_with_count() {
        let mut t = ta_with("abcdef");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(t.text(), "cdef");

        t.input(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE));
        assert_eq!(t.text(), "ef");
    }

    #[test]
    fn vim_mode_failed_find_cancels_operator() {
        let mut t = ta_with("abc");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

        assert_eq!(t.text(), "abc");
        assert_eq!(t.cursor(), 0);
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("NORMAL"));

        t.input(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(t.text(), "abc");
        assert_eq!(t.cursor(), 1);
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("NORMAL"));
    }
    #[test]
    fn vim_mode_undo_redo_basic() {
        let mut t = ta_with("hello");
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        t.set_cursor(0);

        // Insert 'X' at start
        t.input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.text(), "Xhello");

        // Undo should restore original text
        t.input(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(t.text(), "hello");

        // Redo restores insertion
        t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "Xhello");
    }

    #[test]
    fn vim_inner_word_range_basic() {
        let mut t = ta_with("foo bar");
        t.set_cursor(1);
        let range = t.vim_inner_word_range().unwrap();
        assert_eq!(range, (0, 3));

        t.set_cursor(4);
        let range = t.vim_inner_word_range().unwrap();
        assert_eq!(range, (4, 7));
    }

    #[test]
    fn vim_around_word_includes_trailing_space() {
        let mut t = ta_with("foo bar   baz");
        t.set_cursor(5); // inside "bar"
        let range = t.vim_around_word_range().unwrap();
        assert_eq!(range, (4, 10)); // "bar   "
    }

    #[test]
    fn vim_mode_cw_enters_insert() {
        let mut t = ta_with("hello world");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));

        assert_eq!(t.text(), "world");
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("INSERT"));
    }

    #[test]
    fn vim_mode_cc_changes_line_and_enters_insert() {
        let mut t = ta_with("foo\nbar");
        t.set_cursor(0);
        t.set_vim_mode_enabled(true);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        t.input(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        assert_eq!(
            t.text(),
            "
bar"
        );
        assert_eq!(t.cursor(), 0);
        assert_eq!(t.vim_mode_state_label().as_deref(), Some("INSERT"));
    }

    #[test]
    fn delete_backward_word_alt_keys() {
        // Test the custom Alt+Ctrl+h binding
        let mut t = ta_with("hello world");
        t.set_cursor(t.text().len()); // cursor at the end
        t.input(KeyEvent::new(
            KeyCode::Char('h'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));
        assert_eq!(t.text(), "hello ");
        assert_eq!(t.cursor(), 6);

        // Test the standard Alt+Backspace binding
        let mut t = ta_with("hello world");
        t.set_cursor(t.text().len()); // cursor at the end
        t.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(t.text(), "hello ");
        assert_eq!(t.cursor(), 6);
    }

    #[test]
    fn delete_forward_word_with_without_alt_modifier() {
        let mut t = ta_with("hello world");
        t.set_cursor(0);
        t.input(KeyEvent::new(KeyCode::Delete, KeyModifiers::ALT));
        assert_eq!(t.text(), " world");
        assert_eq!(t.cursor(), 0);

        let mut t = ta_with("hello");
        t.set_cursor(0);
        t.input(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        assert_eq!(t.text(), "ello");
        assert_eq!(t.cursor(), 0);
    }

    #[test]
    fn control_h_backspace() {
        // Test Ctrl+H as backspace
        let mut t = ta_with("12345");
        t.set_cursor(3); // cursor after '3'
        t.input(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "1245");
        assert_eq!(t.cursor(), 2);

        // Test Ctrl+H at beginning (should be no-op)
        t.set_cursor(0);
        t.input(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "1245");
        assert_eq!(t.cursor(), 0);

        // Test Ctrl+H at end
        t.set_cursor(t.text().len());
        t.input(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "124");
        assert_eq!(t.cursor(), 3);
    }

    #[test]
    fn cursor_vertical_movement_across_lines_and_bounds() {
        let mut t = ta_with("short\nloooooooooong\nmid");
        // Place cursor on second line, column 5
        let second_line_start = 6; // after first '\n'
        t.set_cursor(second_line_start + 5);

        // Move up: target column preserved, clamped by line length
        t.move_cursor_up();
        assert_eq!(t.cursor(), 5); // first line has len 5

        // Move up again goes to start of text
        t.move_cursor_up();
        assert_eq!(t.cursor(), 0);

        // Move down: from start to target col tracked
        t.move_cursor_down();
        // On first move down, we should land on second line, at col 0 (target col remembered as 0)
        let pos_after_down = t.cursor();
        assert!(pos_after_down >= second_line_start);

        // Move down again to third line; clamp to its length
        t.move_cursor_down();
        let third_line_start = t.text().find("mid").unwrap();
        let third_line_end = third_line_start + 3;
        assert!(t.cursor() >= third_line_start && t.cursor() <= third_line_end);

        // Moving down at last line jumps to end
        t.move_cursor_down();
        assert_eq!(t.cursor(), t.text().len());
    }

    #[test]
    fn home_end_and_default_style_home_end() {
        let mut t = ta_with("one\ntwo\nthree");
        // Position at middle of second line
        let second_line_start = t.text().find("two").unwrap();
        t.set_cursor(second_line_start + 1);

        t.move_cursor_to_beginning_of_line(false);
        assert_eq!(t.cursor(), second_line_start);

        // Ctrl-A behavior: if at BOL, go to beginning of previous line
        t.move_cursor_to_beginning_of_line(true);
        assert_eq!(t.cursor(), 0); // beginning of first line

        // Move to EOL of first line
        t.move_cursor_to_end_of_line(false);
        assert_eq!(t.cursor(), 3);

        // Ctrl-E: if at EOL, go to end of next line
        t.move_cursor_to_end_of_line(true);
        // end of second line ("two") is right before its '\n'
        let end_second_nl = t.text().find("\nthree").unwrap();
        assert_eq!(t.cursor(), end_second_nl);
    }

    #[test]
    fn end_of_line_or_down_at_end_of_text() {
        let mut t = ta_with("one\ntwo");
        // Place cursor at absolute end of the text
        t.set_cursor(t.text().len());
        // Should remain at end without panicking
        t.move_cursor_to_end_of_line(true);
        assert_eq!(t.cursor(), t.text().len());

        // Also verify behavior when at EOL of a non-final line:
        let eol_first_line = 3; // index of '\n' in "one\ntwo"
        t.set_cursor(eol_first_line);
        t.move_cursor_to_end_of_line(true);
        assert_eq!(t.cursor(), t.text().len()); // moves to end of next (last) line
    }

    #[test]
    fn word_navigation_helpers() {
        let t = ta_with("  alpha  beta   gamma");
        let mut t = t; // make mutable for set_cursor
        // Put cursor after "alpha"
        let after_alpha = t.text().find("alpha").unwrap() + "alpha".len();
        t.set_cursor(after_alpha);
        assert_eq!(t.beginning_of_previous_word(), 2); // skip initial spaces

        // Put cursor at start of beta
        let beta_start = t.text().find("beta").unwrap();
        t.set_cursor(beta_start);
        assert_eq!(t.end_of_next_word(), beta_start + "beta".len());

        // If at end, end_of_next_word returns len
        t.set_cursor(t.text().len());
        assert_eq!(t.end_of_next_word(), t.text().len());
    }

    #[test]
    fn wrapping_and_cursor_positions() {
        let mut t = ta_with("hello world here");
        let area = Rect::new(0, 0, 6, 10); // width 6 -> wraps words
        // desired height counts wrapped lines
        assert!(t.desired_height(area.width) >= 3);

        // Place cursor in "world"
        let world_start = t.text().find("world").unwrap();
        t.set_cursor(world_start + 3);
        let (_x, y) = t.cursor_pos(area).unwrap();
        assert_eq!(y, 1); // world should be on second wrapped line

        // With state and small height, cursor is mapped onto visible row
        let mut state = TextAreaState::default();
        let small_area = Rect::new(0, 0, 6, 1);
        // First call: cursor not visible -> effective scroll ensures it is
        let (_x, y) = t.cursor_pos_with_state(small_area, state).unwrap();
        assert_eq!(y, 0);

        // Render with state to update actual scroll value
        let mut buf = Buffer::empty(small_area);
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), small_area, &mut buf, &mut state);
        // After render, state.scroll should be adjusted so cursor row fits
        let effective_lines = t.desired_height(small_area.width);
        assert!(state.scroll < effective_lines);
    }

    #[test]
    fn cursor_pos_with_state_basic_and_scroll_behaviors() {
        // Case 1: No wrapping needed, height fits — scroll ignored, y maps directly.
        let mut t = ta_with("hello world");
        t.set_cursor(3);
        let area = Rect::new(2, 5, 20, 3);
        // Even if an absurd scroll is provided, when content fits the area the
        // effective scroll is 0 and the cursor position matches cursor_pos.
        let bad_state = TextAreaState { scroll: 999 };
        let (x1, y1) = t.cursor_pos(area).unwrap();
        let (x2, y2) = t.cursor_pos_with_state(area, bad_state).unwrap();
        assert_eq!((x2, y2), (x1, y1));

        // Case 2: Cursor below the current window — y should be clamped to the
        // bottom row (area.height - 1) after adjusting effective scroll.
        let mut t = ta_with("one two three four five six");
        // Force wrapping to many visual lines.
        let wrap_width = 4;
        let _ = t.desired_height(wrap_width);
        // Put cursor somewhere near the end so it's definitely below the first window.
        t.set_cursor(t.text().len().saturating_sub(2));
        let small_area = Rect::new(0, 0, wrap_width, 2);
        let state = TextAreaState { scroll: 0 };
        let (_x, y) = t.cursor_pos_with_state(small_area, state).unwrap();
        assert_eq!(y, small_area.y + small_area.height - 1);

        // Case 3: Cursor above the current window — y should be top row (0)
        // when the provided scroll is too large.
        let mut t = ta_with("alpha beta gamma delta epsilon zeta");
        let wrap_width = 5;
        let lines = t.desired_height(wrap_width);
        // Place cursor near start so an excessive scroll moves it to top row.
        t.set_cursor(1);
        let area = Rect::new(0, 0, wrap_width, 3);
        let state = TextAreaState {
            scroll: lines.saturating_mul(2),
        };
        let (_x, y) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!(y, area.y);
    }

    #[test]
    fn wrapped_navigation_across_visual_lines() {
        let mut t = ta_with("abcdefghij");
        // Force wrapping at width 4: lines -> ["abcd", "efgh", "ij"]
        let _ = t.desired_height(4);

        // From the very start, moving down should go to the start of the next wrapped line (index 4)
        t.set_cursor(0);
        t.move_cursor_down();
        assert_eq!(t.cursor(), 4);

        // Cursor at boundary index 4 should be displayed at start of second wrapped line
        t.set_cursor(4);
        let area = Rect::new(0, 0, 4, 10);
        let (x, y) = t.cursor_pos(area).unwrap();
        assert_eq!((x, y), (0, 1));

        // With state and small height, cursor should be visible at row 0, col 0
        let small_area = Rect::new(0, 0, 4, 1);
        let state = TextAreaState::default();
        let (x, y) = t.cursor_pos_with_state(small_area, state).unwrap();
        assert_eq!((x, y), (0, 0));

        // Place cursor in the middle of the second wrapped line ("efgh"), at 'g'
        t.set_cursor(6);
        // Move up should go to same column on previous wrapped line -> index 2 ('c')
        t.move_cursor_up();
        assert_eq!(t.cursor(), 2);

        // Move down should return to same position on the next wrapped line -> back to index 6 ('g')
        t.move_cursor_down();
        assert_eq!(t.cursor(), 6);

        // Move down again should go to third wrapped line. Target col is 2, but the line has len 2 -> clamp to end
        t.move_cursor_down();
        assert_eq!(t.cursor(), t.text().len());
    }

    #[test]
    fn cursor_pos_with_state_after_movements() {
        let mut t = ta_with("abcdefghij");
        // Wrap width 4 -> visual lines: abcd | efgh | ij
        let _ = t.desired_height(4);
        let area = Rect::new(0, 0, 4, 2);
        let mut state = TextAreaState::default();
        let mut buf = Buffer::empty(area);

        // Start at beginning
        t.set_cursor(0);
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x, y) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x, y), (0, 0));

        // Move down to second visual line; should be at bottom row (row 1) within 2-line viewport
        t.move_cursor_down();
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x, y) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x, y), (0, 1));

        // Move down to third visual line; viewport scrolls and keeps cursor on bottom row
        t.move_cursor_down();
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x, y) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x, y), (0, 1));

        // Move up to second visual line; with current scroll, it appears on top row
        t.move_cursor_up();
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x, y) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x, y), (0, 0));

        // Column preservation across moves: set to col 2 on first line, move down
        t.set_cursor(2);
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x0, y0) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x0, y0), (2, 0));
        t.move_cursor_down();
        ratatui::widgets::StatefulWidgetRef::render_ref(&(&t), area, &mut buf, &mut state);
        let (x1, y1) = t.cursor_pos_with_state(area, state).unwrap();
        assert_eq!((x1, y1), (2, 1));
    }

    #[test]
    fn wrapped_navigation_with_newlines_and_spaces() {
        // Include spaces and an explicit newline to exercise boundaries
        let mut t = ta_with("word1  word2\nword3");
        // Width 6 will wrap "word1  " and then "word2" before the newline
        let _ = t.desired_height(6);

        // Put cursor on the second wrapped line before the newline, at column 1 of "word2"
        let start_word2 = t.text().find("word2").unwrap();
        t.set_cursor(start_word2 + 1);

        // Up should go to first wrapped line, column 1 -> index 1
        t.move_cursor_up();
        assert_eq!(t.cursor(), 1);

        // Down should return to the same visual column on "word2"
        t.move_cursor_down();
        assert_eq!(t.cursor(), start_word2 + 1);

        // Down again should cross the logical newline to the next visual line ("word3"), clamped to its length if needed
        t.move_cursor_down();
        let start_word3 = t.text().find("word3").unwrap();
        assert!(t.cursor() >= start_word3 && t.cursor() <= start_word3 + "word3".len());
    }

    #[test]
    fn wrapped_navigation_with_wide_graphemes() {
        // Four thumbs up, each of display width 2, with width 3 to force wrapping inside grapheme boundaries
        let mut t = ta_with("👍👍👍👍");
        let _ = t.desired_height(3);

        // Put cursor after the second emoji (which should be on first wrapped line)
        t.set_cursor("👍👍".len());

        // Move down should go to the start of the next wrapped line (same column preserved but clamped)
        t.move_cursor_down();
        // We expect to land somewhere within the third emoji or at the start of it
        let pos_after_down = t.cursor();
        assert!(pos_after_down >= "👍👍".len());

        // Moving up should take us back to the original position
        t.move_cursor_up();
        assert_eq!(t.cursor(), "👍👍".len());
    }

    #[test]
    fn fuzz_textarea_randomized() {
        // Deterministic seed for reproducibility
        // Seed the RNG based on the current day in Pacific Time (PST/PDT). This
        // keeps the fuzz test deterministic within a day while still varying
        // day-to-day to improve coverage.
        let pst_today_seed: u64 = (chrono::Utc::now() - chrono::Duration::hours(8))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as u64;
        let mut rng = rand::rngs::StdRng::seed_from_u64(pst_today_seed);

        for _case in 0..500 {
            let mut ta = TextArea::new();
            let mut state = TextAreaState::default();
            // Track element payloads we insert. Payloads use characters '[' and ']' which
            // are not produced by rand_grapheme(), avoiding accidental collisions.
            let mut elem_texts: Vec<String> = Vec::new();
            let mut next_elem_id: usize = 0;
            // Start with a random base string
            let base_len = rng.random_range(0..30);
            let mut base = String::new();
            for _ in 0..base_len {
                base.push_str(&rand_grapheme(&mut rng));
            }
            ta.set_text(&base);
            // Choose a valid char boundary for initial cursor
            let mut boundaries: Vec<usize> = vec![0];
            boundaries.extend(ta.text().char_indices().map(|(i, _)| i).skip(1));
            boundaries.push(ta.text().len());
            let init = boundaries[rng.random_range(0..boundaries.len())];
            ta.set_cursor(init);

            let mut width: u16 = rng.random_range(1..=12);
            let mut height: u16 = rng.random_range(1..=4);

            for _step in 0..60 {
                // Mostly stable width/height, occasionally change
                if rng.random_bool(0.1) {
                    width = rng.random_range(1..=12);
                }
                if rng.random_bool(0.1) {
                    height = rng.random_range(1..=4);
                }

                // Pick an operation
                match rng.random_range(0..18) {
                    0 => {
                        // insert small random string at cursor
                        let len = rng.random_range(0..6);
                        let mut s = String::new();
                        for _ in 0..len {
                            s.push_str(&rand_grapheme(&mut rng));
                        }
                        ta.insert_str(&s);
                    }
                    1 => {
                        // replace_range with small random slice
                        let mut b: Vec<usize> = vec![0];
                        b.extend(ta.text().char_indices().map(|(i, _)| i).skip(1));
                        b.push(ta.text().len());
                        let i1 = rng.random_range(0..b.len());
                        let i2 = rng.random_range(0..b.len());
                        let (start, end) = if b[i1] <= b[i2] {
                            (b[i1], b[i2])
                        } else {
                            (b[i2], b[i1])
                        };
                        let insert_len = rng.random_range(0..=4);
                        let mut s = String::new();
                        for _ in 0..insert_len {
                            s.push_str(&rand_grapheme(&mut rng));
                        }
                        let before = ta.text().len();
                        // If the chosen range intersects an element, replace_range will expand to
                        // element boundaries, so the naive size delta assertion does not hold.
                        let intersects_element = elem_texts.iter().any(|payload| {
                            if let Some(pstart) = ta.text().find(payload) {
                                let pend = pstart + payload.len();
                                pstart < end && pend > start
                            } else {
                                false
                            }
                        });
                        ta.replace_range(start..end, &s);
                        if !intersects_element {
                            let after = ta.text().len();
                            assert_eq!(
                                after as isize,
                                before as isize + (s.len() as isize) - ((end - start) as isize)
                            );
                        }
                    }
                    2 => ta.delete_backward(rng.random_range(0..=3)),
                    3 => ta.delete_forward(rng.random_range(0..=3)),
                    4 => ta.delete_backward_word(),
                    5 => ta.kill_to_beginning_of_line(),
                    6 => ta.kill_to_end_of_line(),
                    7 => ta.move_cursor_left(),
                    8 => ta.move_cursor_right(),
                    9 => ta.move_cursor_up(),
                    10 => ta.move_cursor_down(),
                    11 => ta.move_cursor_to_beginning_of_line(true),
                    12 => ta.move_cursor_to_end_of_line(true),
                    13 => {
                        // Insert an element with a unique sentinel payload
                        let payload =
                            format!("[[EL#{}:{}]]", next_elem_id, rng.random_range(1000..9999));
                        next_elem_id += 1;
                        ta.insert_element(&payload);
                        elem_texts.push(payload);
                    }
                    14 => {
                        // Try inserting inside an existing element (should clamp to boundary)
                        if let Some(payload) = elem_texts.choose(&mut rng).cloned()
                            && let Some(start) = ta.text().find(&payload)
                        {
                            let end = start + payload.len();
                            if end - start > 2 {
                                let pos = rng.random_range(start + 1..end - 1);
                                let ins = rand_grapheme(&mut rng);
                                ta.insert_str_at(pos, &ins);
                            }
                        }
                    }
                    15 => {
                        // Replace a range that intersects an element -> whole element should be replaced
                        if let Some(payload) = elem_texts.choose(&mut rng).cloned()
                            && let Some(start) = ta.text().find(&payload)
                        {
                            let end = start + payload.len();
                            // Create an intersecting range [start-δ, end-δ2)
                            let mut s = start.saturating_sub(rng.random_range(0..=2));
                            let mut e = (end + rng.random_range(0..=2)).min(ta.text().len());
                            // Align to char boundaries to satisfy String::replace_range contract
                            let txt = ta.text();
                            while s > 0 && !txt.is_char_boundary(s) {
                                s -= 1;
                            }
                            while e < txt.len() && !txt.is_char_boundary(e) {
                                e += 1;
                            }
                            if s < e {
                                // Small replacement text
                                let mut srep = String::new();
                                for _ in 0..rng.random_range(0..=2) {
                                    srep.push_str(&rand_grapheme(&mut rng));
                                }
                                ta.replace_range(s..e, &srep);
                            }
                        }
                    }
                    16 => {
                        // Try setting the cursor to a position inside an element; it should clamp out
                        if let Some(payload) = elem_texts.choose(&mut rng).cloned()
                            && let Some(start) = ta.text().find(&payload)
                        {
                            let end = start + payload.len();
                            if end - start > 2 {
                                let pos = rng.random_range(start + 1..end - 1);
                                ta.set_cursor(pos);
                            }
                        }
                    }
                    _ => {
                        // Jump to word boundaries
                        if rng.random_bool(0.5) {
                            let p = ta.beginning_of_previous_word();
                            ta.set_cursor(p);
                        } else {
                            let p = ta.end_of_next_word();
                            ta.set_cursor(p);
                        }
                    }
                }

                // Sanity invariants
                assert!(ta.cursor() <= ta.text().len());

                // Element invariants
                for payload in &elem_texts {
                    if let Some(start) = ta.text().find(payload) {
                        let end = start + payload.len();
                        // 1) Text inside elements matches the initially set payload
                        assert_eq!(&ta.text()[start..end], payload);
                        // 2) Cursor is never strictly inside an element
                        let c = ta.cursor();
                        assert!(
                            c <= start || c >= end,
                            "cursor inside element: {start}..{end} at {c}"
                        );
                    }
                }

                // Render and compute cursor positions; ensure they are in-bounds and do not panic
                let area = Rect::new(0, 0, width, height);
                // Stateless render into an area tall enough for all wrapped lines
                let total_lines = ta.desired_height(width);
                let full_area = Rect::new(0, 0, width, total_lines.max(1));
                let mut buf = Buffer::empty(full_area);
                ratatui::widgets::WidgetRef::render_ref(&(&ta), full_area, &mut buf);

                // cursor_pos: x must be within width when present
                let _ = ta.cursor_pos(area);

                // cursor_pos_with_state: always within viewport rows
                let (_x, _y) = ta
                    .cursor_pos_with_state(area, state)
                    .unwrap_or((area.x, area.y));

                // Stateful render should not panic, and updates scroll
                let mut sbuf = Buffer::empty(area);
                ratatui::widgets::StatefulWidgetRef::render_ref(
                    &(&ta),
                    area,
                    &mut sbuf,
                    &mut state,
                );

                // After wrapping, desired height equals the number of lines we would render without scroll
                let total_lines = total_lines as usize;
                // state.scroll must not exceed total_lines when content fits within area height
                if (height as usize) >= total_lines {
                    assert_eq!(state.scroll, 0);
                }
            }
        }
    }
}
