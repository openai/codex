//! Search input widget with mode selector.
//!
//! Provides a query input field with search mode selection and history.

use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::events::SearchMode;

/// Maximum number of history entries to keep.
const MAX_HISTORY: usize = 50;

/// Search input widget state.
#[derive(Debug, Clone, Default)]
pub struct SearchInputState {
    /// Current query text.
    pub query: String,
    /// Cursor position in the query.
    pub cursor: usize,
    /// Current search mode.
    pub mode: SearchMode,
    /// Whether the input is focused.
    pub focused: bool,
    /// Query history (newest at back).
    pub history: VecDeque<String>,
    /// Current history navigation index (None = editing new query).
    history_index: Option<usize>,
    /// Saved query when navigating history.
    saved_query: String,
}

impl SearchInputState {
    /// Create a new search input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a character at the cursor position.
    pub fn insert(&mut self, c: char) {
        self.query.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev_char_boundary = self.query[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.remove(prev_char_boundary);
            self.cursor = prev_char_boundary;
        }
    }

    /// Delete the character at the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.query.len() {
            self.query.remove(self.cursor);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.query[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor < self.query.len() {
            self.cursor = self.query[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.query.len());
        }
    }

    /// Move cursor to start.
    pub fn move_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn move_end(&mut self) {
        self.cursor = self.query.len();
    }

    /// Clear the query.
    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor = 0;
    }

    /// Set the query text.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.cursor = self.query.len();
    }

    /// Cycle to the next search mode.
    pub fn next_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Hybrid => SearchMode::Bm25,
            SearchMode::Bm25 => SearchMode::Vector,
            SearchMode::Vector => SearchMode::Snippet,
            SearchMode::Snippet => SearchMode::Hybrid,
        };
    }

    /// Cycle to the previous search mode.
    pub fn prev_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Hybrid => SearchMode::Snippet,
            SearchMode::Bm25 => SearchMode::Hybrid,
            SearchMode::Vector => SearchMode::Bm25,
            SearchMode::Snippet => SearchMode::Vector,
        };
    }

    /// Push the current query to history.
    pub fn push_history(&mut self) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }

        // Remove duplicate if exists
        self.history.retain(|h| h != &query);

        // Add to history
        self.history.push_back(query);

        // Limit history size
        while self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }

        // Reset history navigation
        self.history_index = None;
        self.saved_query.clear();
    }

    /// Navigate to previous history entry (older).
    pub fn prev_history(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Save current query and start navigation
                self.saved_query = self.query.clone();
                let idx = self.history.len() - 1;
                self.history_index = Some(idx);
                if let Some(query) = self.history.get(idx) {
                    self.query = query.clone();
                    self.cursor = self.query.len();
                }
            }
            Some(idx) if idx > 0 => {
                // Go to older entry
                let new_idx = idx - 1;
                self.history_index = Some(new_idx);
                if let Some(query) = self.history.get(new_idx) {
                    self.query = query.clone();
                    self.cursor = self.query.len();
                }
            }
            _ => {
                // Already at oldest, do nothing
            }
        }
    }

    /// Navigate to next history entry (newer).
    pub fn next_history(&mut self) {
        match self.history_index {
            Some(idx) if idx + 1 < self.history.len() => {
                // Go to newer entry
                let new_idx = idx + 1;
                self.history_index = Some(new_idx);
                if let Some(query) = self.history.get(new_idx) {
                    self.query = query.clone();
                    self.cursor = self.query.len();
                }
            }
            Some(_) => {
                // Restore saved query
                self.query = self.saved_query.clone();
                self.cursor = self.query.len();
                self.history_index = None;
                self.saved_query.clear();
            }
            None => {
                // Not navigating history, do nothing
            }
        }
    }

    /// Reset history navigation (called when user types).
    pub fn reset_history_navigation(&mut self) {
        if self.history_index.is_some() {
            self.history_index = None;
            self.saved_query.clear();
        }
    }

    /// Check if currently navigating history.
    pub fn is_navigating_history(&self) -> bool {
        self.history_index.is_some()
    }
}

/// Search input widget.
pub struct SearchInput<'a> {
    state: &'a SearchInputState,
}

impl<'a> SearchInput<'a> {
    /// Create a new search input widget.
    pub fn new(state: &'a SearchInputState) -> Self {
        Self { state }
    }

    fn mode_indicator(&self) -> Line<'static> {
        let modes = [
            (SearchMode::Hybrid, "Hybrid"),
            (SearchMode::Bm25, "BM25"),
            (SearchMode::Vector, "Vector"),
            (SearchMode::Snippet, "Snippet"),
        ];

        let spans: Vec<Span> = modes
            .iter()
            .enumerate()
            .flat_map(|(i, (mode, name))| {
                let is_selected = *mode == self.state.mode;
                let style = if is_selected {
                    Style::default().add_modifier(Modifier::BOLD).cyan()
                } else {
                    Style::default().dim()
                };

                let prefix = if is_selected { "[" } else { " " };
                let suffix = if is_selected { "]" } else { " " };

                let mut result = vec![
                    Span::styled(prefix, style),
                    Span::styled(*name, style),
                    Span::styled(suffix, style),
                ];

                if i < modes.len() - 1 {
                    result.push(Span::raw(" "));
                }

                result
            })
            .collect();

        Line::from(spans)
    }
}

impl Widget for SearchInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split into mode selector and input areas
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(3)]).split(area);

        // Render mode selector
        let mode_line = self.mode_indicator();
        let mode_para = Paragraph::new(mode_line);
        mode_para.render(chunks[0], buf);

        // Render input box
        let border_style = if self.state.focused {
            Style::default().cyan()
        } else {
            Style::default().dim()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Query ");

        let inner = block.inner(chunks[1]);
        block.render(chunks[1], buf);

        // Render query text with cursor
        let query = &self.state.query;
        let cursor_pos = self.state.cursor;

        if self.state.focused {
            // Show cursor
            let before_cursor = &query[..cursor_pos];
            let at_cursor = query
                .chars()
                .nth(before_cursor.chars().count())
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after_cursor = if cursor_pos < query.len() {
                &query[cursor_pos + at_cursor.len()..]
            } else {
                ""
            };

            let line = Line::from(vec![
                Span::raw(before_cursor),
                Span::styled(at_cursor, Style::default().add_modifier(Modifier::REVERSED)),
                Span::raw(after_cursor),
            ]);
            Paragraph::new(line).render(inner, buf);
        } else {
            // No cursor
            let display = if query.is_empty() {
                Span::styled("Type to search...", Style::default().dim().italic())
            } else {
                Span::raw(query.as_str())
            };
            Paragraph::new(display).render(inner, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_input_state_insert() {
        let mut state = SearchInputState::new();
        state.insert('h');
        state.insert('e');
        state.insert('l');
        state.insert('l');
        state.insert('o');
        assert_eq!(state.query, "hello");
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn test_search_input_state_backspace() {
        let mut state = SearchInputState::new();
        state.set_query("hello".to_string());
        state.backspace();
        assert_eq!(state.query, "hell");
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn test_search_input_state_move() {
        let mut state = SearchInputState::new();
        state.set_query("hello".to_string());
        state.move_left();
        assert_eq!(state.cursor, 4);
        state.move_start();
        assert_eq!(state.cursor, 0);
        state.move_end();
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn test_search_mode_cycle() {
        let mut state = SearchInputState::new();
        assert_eq!(state.mode, SearchMode::Hybrid);
        state.next_mode();
        assert_eq!(state.mode, SearchMode::Bm25);
        state.next_mode();
        assert_eq!(state.mode, SearchMode::Vector);
        state.prev_mode();
        assert_eq!(state.mode, SearchMode::Bm25);
    }

    #[test]
    fn test_history_push() {
        let mut state = SearchInputState::new();
        state.set_query("first".to_string());
        state.push_history();
        state.set_query("second".to_string());
        state.push_history();

        assert_eq!(state.history.len(), 2);
        assert_eq!(state.history[0], "first");
        assert_eq!(state.history[1], "second");
    }

    #[test]
    fn test_history_dedup() {
        let mut state = SearchInputState::new();
        state.set_query("query".to_string());
        state.push_history();
        state.set_query("query".to_string());
        state.push_history();

        // Duplicate should be removed, only one entry
        assert_eq!(state.history.len(), 1);
    }

    #[test]
    fn test_history_navigation() {
        let mut state = SearchInputState::new();
        state.set_query("first".to_string());
        state.push_history();
        state.set_query("second".to_string());
        state.push_history();
        state.set_query("current".to_string());

        // Navigate to previous (second)
        state.prev_history();
        assert_eq!(state.query, "second");
        assert!(state.is_navigating_history());

        // Navigate to older (first)
        state.prev_history();
        assert_eq!(state.query, "first");

        // Navigate back to newer (second)
        state.next_history();
        assert_eq!(state.query, "second");

        // Navigate back to current
        state.next_history();
        assert_eq!(state.query, "current");
        assert!(!state.is_navigating_history());
    }

    #[test]
    fn test_history_reset_on_type() {
        let mut state = SearchInputState::new();
        state.set_query("first".to_string());
        state.push_history();
        state.set_query("current".to_string());

        // Start navigating
        state.prev_history();
        assert!(state.is_navigating_history());

        // Type something - should reset navigation
        state.reset_history_navigation();
        assert!(!state.is_navigating_history());
    }
}
