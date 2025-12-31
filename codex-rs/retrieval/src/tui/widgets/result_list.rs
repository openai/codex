//! Search result list widget.
//!
//! Displays search results with selection and scrolling support.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Widget;

use crate::events::SearchResultSummary;

/// Result list widget state.
#[derive(Debug, Clone, Default)]
pub struct ResultListState {
    /// Search results to display.
    pub results: Vec<SearchResultSummary>,
    /// List selection state.
    pub list_state: ListState,
    /// Whether this widget is focused.
    pub focused: bool,
    /// Search duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Whether a search is in progress.
    pub searching: bool,
    /// Last known visible height (for pagination).
    pub visible_height: usize,
}

impl ResultListState {
    /// Create a new result list state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the search results.
    pub fn set_results(&mut self, results: Vec<SearchResultSummary>, duration_ms: i64) {
        self.results = results;
        self.duration_ms = Some(duration_ms);
        self.searching = false;
        // Select first item if results exist
        if !self.results.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    /// Clear results and mark as searching.
    pub fn start_search(&mut self) {
        self.results.clear();
        self.duration_ms = None;
        self.searching = true;
        self.list_state.select(None);
    }

    /// Get the currently selected result.
    pub fn selected(&self) -> Option<&SearchResultSummary> {
        self.list_state.selected().and_then(|i| self.results.get(i))
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    /// Select the next item.
    pub fn select_next(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.results.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Select the previous item.
    pub fn select_previous(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.results.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Select a specific index.
    pub fn select(&mut self, index: usize) {
        if index < self.results.len() {
            self.list_state.select(Some(index));
        }
    }

    /// Page down (move 10 items).
    pub fn page_down(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = (current + 10).min(self.results.len() - 1);
        self.list_state.select(Some(new_index));
    }

    /// Page up (move 10 items).
    pub fn page_up(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let new_index = current.saturating_sub(10);
        self.list_state.select(Some(new_index));
    }

    /// Go to first item.
    pub fn select_first(&mut self) {
        if !self.results.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Go to last item.
    pub fn select_last(&mut self) {
        if !self.results.is_empty() {
            self.list_state.select(Some(self.results.len() - 1));
        }
    }

    /// Get pagination info (current_page, total_pages).
    /// Returns None if no pagination needed (all items fit on one page).
    pub fn pagination_info(&self) -> Option<(usize, usize)> {
        if self.visible_height == 0 || self.results.is_empty() {
            return None;
        }

        let total_pages = (self.results.len() + self.visible_height - 1) / self.visible_height;
        if total_pages <= 1 {
            return None;
        }

        let current_index = self.list_state.selected().unwrap_or(0);
        let current_page = current_index / self.visible_height + 1;

        Some((current_page, total_pages))
    }
}

/// Result list widget.
pub struct ResultList<'a> {
    state: &'a ResultListState,
}

impl<'a> ResultList<'a> {
    /// Create a new result list widget.
    pub fn new(state: &'a ResultListState) -> Self {
        Self { state }
    }

    fn format_result_item(&self, index: usize, result: &SearchResultSummary) -> ListItem<'static> {
        let is_selected = self.state.list_state.selected() == Some(index);

        // Format: "1. path/to/file.rs:10-20  [Type]  0.912 ⚠"
        let num = format!("{:>3}. ", index + 1);
        let path = format!(
            "{}:{}-{}",
            result.filepath, result.start_line, result.end_line
        );
        let score_type_str = format!("[{}]", result.score_type);
        let score = format!("{:.3}", result.score);

        let path_style = if is_selected {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        // Color code by score type
        let type_style = match result.score_type {
            crate::types::ScoreType::Bm25 => Style::default().blue(),
            crate::types::ScoreType::Vector => Style::default().magenta(),
            crate::types::ScoreType::Hybrid => Style::default().green(),
            crate::types::ScoreType::Snippet => Style::default().yellow(),
            crate::types::ScoreType::Recent => Style::default().cyan(),
        };

        let score_style = Style::default().cyan();

        // Staleness indicator
        let staleness = match result.is_stale {
            Some(true) => Span::styled(" \u{26a0}", Style::default().yellow()), // Stale
            Some(false) => Span::styled(" \u{2713}", Style::default().green().dim()), // Fresh
            None => Span::raw(""),                                              // Not checked
        };

        // Language tag
        let lang = format!(" ({})", result.language);

        let line = Line::from(vec![
            Span::styled(num, Style::default().dim()),
            Span::styled(path, path_style),
            Span::raw("  "),
            Span::styled(score_type_str, type_style),
            Span::raw("  "),
            Span::styled(score, score_style),
            Span::styled(lang, Style::default().dim()),
            staleness,
        ]);

        ListItem::new(line)
    }
}

impl StatefulWidget for ResultList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate visible height (area height minus borders)
        let visible_height = area.height.saturating_sub(2) as usize;

        // Build title with result count, duration, and pagination
        let title = if self.state.searching {
            " Searching... ".to_string()
        } else if self.state.results.is_empty() {
            " Results (0) ".to_string()
        } else {
            let duration_str = self
                .state
                .duration_ms
                .map(|d| format!(", {}ms", d))
                .unwrap_or_default();

            // Calculate pagination with current visible height
            let total_pages = if visible_height > 0 {
                (self.state.results.len() + visible_height - 1) / visible_height
            } else {
                1
            };

            let page_str = if total_pages > 1 {
                let current_index = self.state.list_state.selected().unwrap_or(0);
                let current_page = current_index / visible_height.max(1) + 1;
                format!(" [Page {}/{}]", current_page, total_pages)
            } else {
                String::new()
            };

            format!(
                " Results ({}{}){} ",
                self.state.results.len(),
                duration_str,
                page_str
            )
        };

        let border_style = if self.state.focused {
            Style::default().cyan()
        } else {
            Style::default().dim()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        if self.state.results.is_empty() {
            // Show empty state message
            let inner = block.inner(area);
            block.render(area, buf);

            let message = if self.state.searching {
                "Searching..."
            } else {
                "No results. Type a query and press Enter to search."
            };

            let msg_line = Line::from(Span::styled(message, Style::default().dim().italic()));
            let para = ratatui::widgets::Paragraph::new(msg_line);
            para.render(inner, buf);
        } else {
            // Build list items
            let items: Vec<ListItem> = self
                .state
                .results
                .iter()
                .enumerate()
                .map(|(i, r)| self.format_result_item(i, r))
                .collect();

            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            // Copy state for rendering
            *state = self.state.list_state.clone();
            StatefulWidget::render(list, area, buf, state);
        }
    }
}

// Also implement Widget for simpler rendering
impl Widget for ResultList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.list_state.clone();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScoreType;

    fn make_result(filepath: &str, score: f32, score_type: ScoreType) -> SearchResultSummary {
        SearchResultSummary {
            filepath: filepath.to_string(),
            start_line: 10,
            end_line: 20,
            score,
            score_type,
            language: "rust".to_string(),
            preview: Some("fn main() {}".to_string()),
            is_stale: None,
        }
    }

    #[test]
    fn test_result_list_state_navigation() {
        let mut state = ResultListState::new();

        let results = vec![
            make_result("a.rs", 0.9, ScoreType::Bm25),
            make_result("b.rs", 0.8, ScoreType::Vector),
            make_result("c.rs", 0.7, ScoreType::Hybrid),
        ];
        state.set_results(results, 100);

        assert_eq!(state.selected_index(), Some(0));

        state.select_next();
        assert_eq!(state.selected_index(), Some(1));

        state.select_next();
        assert_eq!(state.selected_index(), Some(2));

        // Wrap around
        state.select_next();
        assert_eq!(state.selected_index(), Some(0));

        state.select_previous();
        assert_eq!(state.selected_index(), Some(2));
    }

    #[test]
    fn test_result_list_state_page_navigation() {
        let mut state = ResultListState::new();

        let results: Vec<SearchResultSummary> = (0..25)
            .map(|i| {
                make_result(
                    &format!("file{}.rs", i),
                    1.0 - (i as f32 * 0.01),
                    ScoreType::Hybrid,
                )
            })
            .collect();
        state.set_results(results, 200);

        state.page_down();
        assert_eq!(state.selected_index(), Some(10));

        state.page_down();
        assert_eq!(state.selected_index(), Some(20));

        state.page_down();
        assert_eq!(state.selected_index(), Some(24)); // Clamped to last

        state.page_up();
        assert_eq!(state.selected_index(), Some(14));

        state.select_first();
        assert_eq!(state.selected_index(), Some(0));

        state.select_last();
        assert_eq!(state.selected_index(), Some(24));
    }

    #[test]
    fn test_start_search_clears_state() {
        let mut state = ResultListState::new();

        let results = vec![make_result("a.rs", 0.9, ScoreType::Bm25)];
        state.set_results(results, 100);
        assert!(!state.results.is_empty());
        assert!(state.duration_ms.is_some());

        state.start_search();
        assert!(state.results.is_empty());
        assert!(state.duration_ms.is_none());
        assert!(state.searching);
        assert!(state.selected_index().is_none());
    }
}
