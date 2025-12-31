//! Search functionality for the transcript overlay.
//!
//! This module provides text search within the chat transcript, allowing users to
//! find specific text in the conversation history. Search is triggered by pressing
//! `/` or `Ctrl+F` when viewing the transcript overlay (opened with Ctrl+T).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use std::sync::Arc;

use crate::history_cell::HistoryCell;

/// Represents a single search match in the transcript.
#[derive(Debug, Clone)]
pub(crate) struct SearchMatch {
    /// Index of the cell containing this match.
    pub cell_index: usize,
    /// Character offset within the cell's text where the match starts.
    #[allow(dead_code)]
    pub char_offset: usize,
    /// Length of the matched text in characters.
    #[allow(dead_code)]
    pub match_len: usize,
}

/// State for transcript search functionality.
#[derive(Debug)]
pub(crate) struct TranscriptSearch {
    /// Current search query entered by the user.
    query: String,
    /// Whether search mode is active (input bar visible).
    active: bool,
    /// All matches found in the transcript.
    matches: Vec<SearchMatch>,
    /// Index of the currently highlighted match (0-based).
    current_match: Option<usize>,
    /// Cached text content of each cell for searching.
    cell_texts: Vec<String>,
}

impl Default for TranscriptSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptSearch {
    /// Creates a new search state.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active: false,
            matches: Vec::new(),
            current_match: None,
            cell_texts: Vec::new(),
        }
    }

    /// Returns whether search mode is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activates search mode.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivates search mode and clears the query.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current_match = None;
    }

    /// Returns the current search query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the number of matches found.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns the current match index (1-based for display), if any.
    pub fn current_match_display(&self) -> Option<usize> {
        self.current_match.map(|i| i + 1)
    }

    /// Returns the cell index of the current match, if any.
    pub fn current_match_cell(&self) -> Option<usize> {
        self.current_match
            .and_then(|i| self.matches.get(i))
            .map(|m| m.cell_index)
    }

    /// Appends a character to the search query and re-runs the search.
    pub fn push_char(&mut self, c: char, cells: &[Arc<dyn HistoryCell>], width: u16) {
        self.query.push(c);
        self.update_search(cells, width);
    }

    /// Replace the entire query string and re-run the search.
    pub fn set_query(&mut self, query: &str, cells: &[Arc<dyn HistoryCell>], width: u16) {
        self.query.clear();
        self.query.push_str(query);
        self.update_search(cells, width);
    }

    /// Removes the last character from the search query and re-runs the search.
    pub fn pop_char(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) {
        self.query.pop();
        self.update_search(cells, width);
    }

    /// Moves to the next match.
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        });
    }

    /// Moves to the previous match.
    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(0) => self.matches.len() - 1,
            Some(i) => i - 1,
            None => self.matches.len() - 1,
        });
    }

    /// Updates the cached cell texts and re-runs the search.
    fn update_search(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) {
        // Cache cell text content if not already done or if cell count changed
        if self.cell_texts.len() != cells.len() {
            self.cell_texts = cells
                .iter()
                .map(|cell| extract_cell_text(cell, width))
                .collect();
        }

        self.matches.clear();
        self.current_match = None;

        if self.query.is_empty() {
            return;
        }

        let query_lower = self.query.to_lowercase();

        for (cell_index, text) in self.cell_texts.iter().enumerate() {
            let text_lower = text.to_lowercase();
            let mut search_start = 0;

            while let Some(pos) = text_lower[search_start..].find(&query_lower) {
                let char_offset = search_start + pos;
                self.matches.push(SearchMatch {
                    cell_index,
                    char_offset,
                    match_len: self.query.len(),
                });
                search_start = char_offset + 1;
            }
        }

        // Select the first match if any found
        if !self.matches.is_empty() {
            self.current_match = Some(0);
        }
    }

    /// Refreshes the search with updated cells (e.g., when new messages arrive).
    pub fn refresh(&mut self, cells: &[Arc<dyn HistoryCell>], width: u16) {
        // Force re-cache of cell texts
        self.cell_texts.clear();
        if self.active && !self.query.is_empty() {
            self.update_search(cells, width);
        }
    }

    /// Renders the search bar at the given area.
    pub fn render_search_bar(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Search icon/prefix
        spans.push("/".cyan());
        spans.push(" ".into());

        // Query text
        spans.push(Span::from(self.query.clone()));

        // Cursor indicator
        spans.push("▏".dim());

        // Match count
        if !self.query.is_empty() {
            let count_text = if self.matches.is_empty() {
                " (no matches)".to_string()
            } else {
                let current = self.current_match_display().unwrap_or(0);
                format!(" ({}/{})", current, self.matches.len())
            };
            spans.push(count_text.dim());
        }

        let line = Line::from(spans);
        Paragraph::new(vec![line]).render_ref(area, buf);
    }

    /// Returns whether the given cell index contains the current match.
    #[allow(dead_code)]
    pub fn is_current_match_cell(&self, cell_index: usize) -> bool {
        self.current_match
            .and_then(|i| self.matches.get(i))
            .is_some_and(|m| m.cell_index == cell_index)
    }

    /// Returns all matches in the given cell.
    #[allow(dead_code)]
    pub fn matches_in_cell(&self, cell_index: usize) -> Vec<&SearchMatch> {
        self.matches
            .iter()
            .filter(|m| m.cell_index == cell_index)
            .collect()
    }

    /// Returns the style to use for highlighting matched text.
    #[allow(dead_code)]
    pub fn match_style() -> Style {
        Style::default().on_cyan().black()
    }

    /// Returns the style to use for highlighting the current match.
    #[allow(dead_code)]
    pub fn current_match_style() -> Style {
        Style::default().on_magenta().black()
    }
}

/// Extracts plain text content from a history cell for searching.
fn extract_cell_text(cell: &Arc<dyn HistoryCell>, width: u16) -> String {
    let lines = cell.transcript_lines(width);
    let mut text = String::new();
    for line in lines {
        for span in line.spans {
            text.push_str(&span.content);
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    #[derive(Debug)]
    struct TestCell {
        lines: Vec<Line<'static>>,
    }

    impl HistoryCell for TestCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }
    }

    fn make_cell(text: &str) -> Arc<dyn HistoryCell> {
        Arc::new(TestCell {
            lines: vec![Line::from(text.to_string())],
        })
    }

    #[test]
    fn search_finds_matches() {
        let cells = vec![
            make_cell("Hello world"),
            make_cell("This is a test"),
            make_cell("Hello again"),
        ];

        let mut search = TranscriptSearch::new();
        search.activate();

        search.push_char('H', &cells, 80);
        search.push_char('e', &cells, 80);
        search.push_char('l', &cells, 80);
        search.push_char('l', &cells, 80);
        search.push_char('o', &cells, 80);

        assert_eq!(search.match_count(), 2);
        assert_eq!(search.current_match_display(), Some(1));
        assert_eq!(search.current_match_cell(), Some(0));
    }

    #[test]
    fn search_is_case_insensitive() {
        let cells = vec![make_cell("Hello World"), make_cell("HELLO world")];

        let mut search = TranscriptSearch::new();
        search.activate();

        for c in "hello".chars() {
            search.push_char(c, &cells, 80);
        }

        assert_eq!(search.match_count(), 2);
    }

    #[test]
    fn next_prev_match_navigation() {
        let cells = vec![
            make_cell("test one"),
            make_cell("test two"),
            make_cell("test three"),
        ];

        let mut search = TranscriptSearch::new();
        search.activate();

        for c in "test".chars() {
            search.push_char(c, &cells, 80);
        }

        assert_eq!(search.match_count(), 3);
        assert_eq!(search.current_match_display(), Some(1));

        search.next_match();
        assert_eq!(search.current_match_display(), Some(2));

        search.next_match();
        assert_eq!(search.current_match_display(), Some(3));

        search.next_match();
        assert_eq!(search.current_match_display(), Some(1)); // wraps around

        search.prev_match();
        assert_eq!(search.current_match_display(), Some(3)); // wraps backwards
    }

    #[test]
    fn backspace_updates_search() {
        let cells = vec![make_cell("hello"), make_cell("help")];

        let mut search = TranscriptSearch::new();
        search.activate();

        for c in "hello".chars() {
            search.push_char(c, &cells, 80);
        }
        assert_eq!(search.match_count(), 1);

        // Remove 'o' and 'l'
        search.pop_char(&cells, 80);
        search.pop_char(&cells, 80);

        // Now searching for "hel" should match both
        assert_eq!(search.match_count(), 2);
    }

    #[test]
    fn deactivate_clears_state() {
        let cells = vec![make_cell("test")];

        let mut search = TranscriptSearch::new();
        search.activate();
        search.push_char('t', &cells, 80);

        assert!(search.is_active());
        assert!(!search.query().is_empty());

        search.deactivate();

        assert!(!search.is_active());
        assert!(search.query().is_empty());
        assert_eq!(search.match_count(), 0);
    }
}
