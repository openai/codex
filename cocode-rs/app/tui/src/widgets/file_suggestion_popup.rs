//! File suggestion popup widget.
//!
//! Displays a dropdown list of file suggestions for @mention autocomplete.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use crate::state::FileSuggestionState;

/// Maximum number of visible suggestions in the popup.
const MAX_VISIBLE: i32 = 8;

/// File suggestion popup widget.
///
/// Renders a dropdown list of file suggestions below the input area.
pub struct FileSuggestionPopup<'a> {
    state: &'a FileSuggestionState,
}

impl<'a> FileSuggestionPopup<'a> {
    /// Create a new file suggestion popup.
    pub fn new(state: &'a FileSuggestionState) -> Self {
        Self { state }
    }

    /// Calculate the area for the popup based on input position.
    ///
    /// The popup appears below the input widget, anchored to the left,
    /// with enough width to show file paths.
    pub fn calculate_area(&self, input_area: Rect, terminal_height: u16) -> Rect {
        let suggestion_count = self.state.suggestions.len() as i32;
        let visible_count = suggestion_count.min(MAX_VISIBLE);

        // Height: suggestions + 2 for border + 1 for hint line
        let height = (visible_count as u16 + 3).min(terminal_height / 3);

        // Width: Use most of the input area width, or terminal width
        let width = input_area.width.min(60).max(30);

        // Position: below input area
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height); // Above input if needed

        // Ensure we don't go off-screen
        let y = if y + height > terminal_height {
            terminal_height.saturating_sub(height)
        } else {
            y
        };

        Rect::new(x, y, width, height)
    }

    /// Build highlighted text for a suggestion.
    ///
    /// Characters at match_indices are rendered bold.
    fn highlight_path<'b>(path: &'b str, match_indices: &[i32]) -> Line<'b> {
        if match_indices.is_empty() {
            return Line::from(path);
        }

        let mut spans = Vec::new();
        let mut last_end = 0;

        for &idx in match_indices {
            let idx = idx as usize;
            if idx >= path.len() {
                continue;
            }

            // Non-highlighted part before this match
            if idx > last_end {
                spans.push(Span::raw(&path[last_end..idx]));
            }

            // Highlighted character
            let char_end = path[idx..].chars().next().map(|c| idx + c.len_utf8());
            if let Some(end) = char_end {
                spans.push(Span::styled(
                    &path[idx..end],
                    Style::default().bold().cyan(),
                ));
                last_end = end;
            }
        }

        // Remaining part after last match
        if last_end < path.len() {
            spans.push(Span::raw(&path[last_end..]));
        }

        Line::from(spans)
    }
}

impl Widget for FileSuggestionPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        // Clear the popup area
        Clear.render(area, buf);

        // Create border
        let title = format!(" @{} ", self.state.query);
        let block = Block::default()
            .title(title.bold())
            .borders(Borders::ALL)
            .border_style(Style::default().cyan());

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        // Calculate visible range (scrolling)
        let total = self.state.suggestions.len() as i32;
        let selected = self.state.selected;
        let visible = (inner.height as i32 - 1).max(1); // -1 for hint line

        // Calculate scroll offset to keep selected item visible
        let scroll_offset = if selected < visible / 2 {
            0
        } else if selected > total - (visible + 1) / 2 {
            (total - visible).max(0)
        } else {
            selected - visible / 2
        };

        // Render suggestions
        let mut y = inner.y;
        for (i, suggestion) in self
            .state
            .suggestions
            .iter()
            .skip(scroll_offset as usize)
            .take(visible as usize)
            .enumerate()
        {
            if y >= inner.y + inner.height - 1 {
                break;
            }

            let global_idx = scroll_offset + i as i32;
            let is_selected = global_idx == selected;

            // Calculate display area for this item
            let item_area = Rect::new(inner.x, y, inner.width, 1);

            // Style based on selection
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };

            // Clear line with background
            buf.set_style(item_area, style);

            // Build the display text
            let prefix = if is_selected { "▸ " } else { "  " };
            let suffix = if suggestion.is_directory { "/" } else { "" };
            let display = format!("{prefix}{}{suffix}", suggestion.display_text);

            // Render with highlighting if not selected (selection bg is enough)
            if is_selected {
                buf.set_string(inner.x, y, &display, style);
            } else {
                // Apply match highlighting
                buf.set_string(inner.x, y, prefix, style);

                let path_start = inner.x + 2;
                let mut x = path_start;

                for (char_idx, c) in suggestion.display_text.chars().enumerate() {
                    let is_match = suggestion.match_indices.contains(&(char_idx as i32));
                    let char_style = if is_match { style.bold().cyan() } else { style };
                    buf.set_string(x, y, c.to_string(), char_style);
                    x += 1;
                }

                if suggestion.is_directory {
                    buf.set_string(x, y, "/", style.dim());
                }
            }

            y += 1;
        }

        // Render hint line at bottom
        if inner.height > 1 {
            let hint_y = inner.y + inner.height - 1;
            let hint = if self.state.loading {
                "Searching..."
            } else if self.state.suggestions.is_empty() {
                "No matches"
            } else {
                "Tab/Enter: Accept  Esc: Dismiss"
            };
            buf.set_string(inner.x, hint_y, hint, Style::default().dim());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::FileSuggestionItem;

    fn create_test_state() -> FileSuggestionState {
        let mut state = FileSuggestionState::new("src/".to_string(), 0);
        state.update_suggestions(vec![
            FileSuggestionItem {
                path: "src/main.rs".to_string(),
                display_text: "src/main.rs".to_string(),
                score: 100,
                match_indices: vec![0, 1, 2, 4, 5, 6, 7],
                is_directory: false,
            },
            FileSuggestionItem {
                path: "src/lib.rs".to_string(),
                display_text: "src/lib.rs".to_string(),
                score: 90,
                match_indices: vec![0, 1, 2],
                is_directory: false,
            },
            FileSuggestionItem {
                path: "src/utils/".to_string(),
                display_text: "src/utils".to_string(),
                score: 80,
                match_indices: vec![0, 1, 2],
                is_directory: true,
            },
        ]);
        state
    }

    #[test]
    fn test_popup_creation() {
        let state = create_test_state();
        let popup = FileSuggestionPopup::new(&state);

        let input_area = Rect::new(0, 20, 80, 3);
        let area = popup.calculate_area(input_area, 24);

        assert!(area.width >= 30);
        assert!(area.height >= 3);
    }

    #[test]
    fn test_popup_render() {
        let state = create_test_state();
        let popup = FileSuggestionPopup::new(&state);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        popup.render(area, &mut buf);

        // Should contain the query
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("src/"));
    }

    #[test]
    fn test_highlight_path() {
        let path = "src/main.rs";
        let indices = vec![0, 4, 5];

        let line = FileSuggestionPopup::highlight_path(path, &indices);

        // Should have multiple spans (highlighted and non-highlighted)
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_empty_suggestions() {
        let mut state = FileSuggestionState::new("xyz".to_string(), 0);
        // Update with empty suggestions to mark loading as false
        state.update_suggestions(vec![]);
        let popup = FileSuggestionPopup::new(&state);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        popup.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("No matches"));
    }

    #[test]
    fn test_loading_state() {
        let state = FileSuggestionState::new("src".to_string(), 0);
        // loading is true by default
        let popup = FileSuggestionPopup::new(&state);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        popup.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Searching"));
    }
}
