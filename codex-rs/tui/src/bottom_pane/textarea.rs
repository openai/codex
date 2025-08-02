use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::WidgetRef;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
pub(crate) struct TextArea {
    text: String,
    cursor_pos: usize,
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len());
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert_str(&mut self, text: &str) {
        self.insert_str_at(self.cursor_pos, text);
    }

    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        self.text.insert_str(pos, text);
        if pos <= self.cursor_pos {
            self.cursor_pos += text.len();
        }
    }

    pub fn replace_range(&mut self, range: std::ops::Range<usize>, text: &str) {
        // Capture the information we need **before** mutating `self.text`.
        assert!(range.start <= range.end);
        let start = range.start.clamp(0, self.text.len());
        let end = range.end.clamp(0, self.text.len());
        let removed_len = end - start;
        let inserted_len = text.len();
        let diff = inserted_len as isize - removed_len as isize;

        // Perform the actual replacement.
        self.text.replace_range(range, text);

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
        .min(self.text.len()); // Clamp just in case
    }

    pub fn cursor(&self) -> usize {
        self.cursor_pos
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor_pos = pos.clamp(0, self.text.len());
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        textwrap::wrap(&self.text, width as usize).len() as u16
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let lines = textwrap::wrap(&self.text, area.width as usize);
        for (i, line) in lines.iter().enumerate() {
            match line {
                std::borrow::Cow::Borrowed(line) => {
                    let line_start =
                        unsafe { line.as_ptr().offset_from(self.text.as_ptr()) as usize };
                    let line_end = line_start
                        + line.len()
                        + self.text[line_start + line.len()..]
                            .chars()
                            .take_while(|c| *c == ' ')
                            .count();
                    if (line_start <= self.cursor_pos && self.cursor_pos < line_end)
                        || (line_end == self.text.len() && self.cursor_pos == line_end)
                    {
                        let col = self.text[line_start..self.cursor_pos].width() as u16;
                        return Some((area.x + col, area.y + i as u16));
                    }
                }
                std::borrow::Cow::Owned(_) => unreachable!(),
            }
        }
        unreachable!()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn beginning_of_current_line(&self) -> usize {
        self.text[..self.cursor_pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn end_of_current_line(&self) -> usize {
        self.text[self.cursor_pos..]
            .find('\n')
            .map(|i| i + self.cursor_pos)
            .unwrap_or(self.text.len())
    }

    fn beginning_of_previous_word(&self) -> usize {
        if let Some(first_non_ws) = self.text[..self.cursor_pos].rfind(|c: char| !c.is_whitespace())
        {
            self.text[..first_non_ws]
                .rfind(|c: char| c.is_whitespace())
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn end_of_next_word(&self) -> usize {
        let Some(first_non_ws) = self.text[self.cursor_pos..].find(|c: char| !c.is_whitespace())
        else {
            return self.text.len();
        };
        let word_start = self.cursor_pos + first_non_ws;
        match self.text[word_start..].find(|c: char| c.is_whitespace()) {
            Some(rel_idx) => word_start + rel_idx,
            None => self.text.len(),
        }
    }

    pub fn input(&mut self, event: KeyEvent) {
        match event {
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT | KeyModifiers::ALT,
                ..
            } => self.insert_str(&c.to_string()),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.insert_str("\n"),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => self.replace_range(self.cursor_pos.saturating_sub(1)..self.cursor_pos, ""),
            KeyEvent {
                code: KeyCode::Delete,
                ..
            } => self.replace_range(self.cursor_pos..self.cursor_pos + 1, ""),

            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.replace_range(self.beginning_of_previous_word()..self.cursor_pos, "");
            }
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.cursor_pos > 0 {
                    let bol = self.beginning_of_current_line();
                    if self.cursor_pos == bol {
                        self.replace_range(bol - 1..bol, "");
                    } else {
                        self.replace_range(bol..self.cursor_pos, "");
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.cursor_pos < self.text.len() {
                    let eol = self.text[self.cursor_pos..]
                        .find('\n')
                        .map(|i| i + self.cursor_pos)
                        .unwrap_or(self.text.len());
                    if self.cursor_pos == eol {
                        self.replace_range(self.cursor_pos..eol + 1, "");
                    } else {
                        self.replace_range(self.cursor_pos..eol, "");
                    }
                }
            }

            // Cursor movement
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Move the cursor left by a single grapheme cluster
                // rather than a single byte.
                let mut gc = unicode_segmentation::GraphemeCursor::new(
                    self.cursor_pos,
                    self.text.len(),
                    false,
                );
                match gc.prev_boundary(&self.text, 0) {
                    Ok(Some(boundary)) => self.cursor_pos = boundary,
                    Ok(None) => self.cursor_pos = 0, // Already at start.
                    Err(_) => self.cursor_pos = self.cursor_pos.saturating_sub(1),
                }
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let mut gc = unicode_segmentation::GraphemeCursor::new(
                    self.cursor_pos,
                    self.text.len(),
                    false,
                );
                match gc.next_boundary(&self.text, 0) {
                    Ok(Some(boundary)) => self.cursor_pos = boundary,
                    Ok(None) => self.cursor_pos = self.text.len(), // Already at end.
                    Err(_) => self.cursor_pos = self.cursor_pos.saturating_add(1),
                }
            }
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                if let Some(prev_nl) = self.text[..self.cursor_pos].rfind('\n') {
                    let cursor_column = self.text[prev_nl..self.cursor_pos].width();
                    let prev_line_start = self.text[..prev_nl].rfind('\n').unwrap_or(0);
                    let mut width_so_far = 0;
                    for (i, w) in self.text[prev_line_start..prev_nl].grapheme_indices(true) {
                        width_so_far += w.width();
                        if width_so_far > cursor_column {
                            self.cursor_pos = prev_line_start + i;
                            return;
                        }
                    }
                    self.cursor_pos = prev_nl;
                } else {
                    self.cursor_pos = 0;
                }
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                let prev_nl = self.text[..self.cursor_pos]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let cursor_column = self.text[prev_nl..self.cursor_pos].width();
                if let Some(next_nl) = self.text[self.cursor_pos..]
                    .find('\n')
                    .map(|i| i + self.cursor_pos)
                {
                    let next_line_end = self.text[next_nl + 1..]
                        .find('\n')
                        .map(|i| i + next_nl + 1)
                        .unwrap_or(self.text.len());
                    let mut width_so_far = 0;
                    for (i, w) in self.text[next_nl + 1..next_line_end].grapheme_indices(true) {
                        width_so_far += w.width();
                        if width_so_far > cursor_column {
                            self.cursor_pos = next_nl + 1 + i;
                            return;
                        }
                    }
                    self.cursor_pos = next_line_end;
                } else {
                    self.cursor_pos = self.text.len();
                }
            }
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.cursor_pos = self.beginning_of_current_line();
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let bol = self.beginning_of_current_line();
                if self.cursor_pos == bol {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                    self.cursor_pos = self.beginning_of_current_line();
                } else {
                    self.cursor_pos = bol;
                }
            }

            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.cursor_pos = self.end_of_current_line();
            }
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                let eol = self.end_of_current_line();
                if self.cursor_pos == eol {
                    self.cursor_pos = (self.cursor_pos + 1).min(self.text.len());
                    self.cursor_pos = self.end_of_current_line();
                } else {
                    self.cursor_pos = eol;
                }
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.cursor_pos = self.beginning_of_previous_word();
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.cursor_pos = self.end_of_next_word();
            }
            o => {
                tracing::info!("Other: {:?}", o);
            }
        }
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len());
    }
}

impl WidgetRef for &TextArea {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        for (i, line) in textwrap::wrap(&self.text, area.width as usize)
            .iter()
            .enumerate()
        {
            buf.set_string(area.x, area.y + i as u16, line, Style::default());
        }
    }
}
