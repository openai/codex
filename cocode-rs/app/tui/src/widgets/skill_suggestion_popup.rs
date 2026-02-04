//! Skill suggestion popup widget.
//!
//! Displays a dropdown list of skill suggestions for /command autocomplete.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use crate::state::SkillSuggestionState;

/// Maximum number of visible suggestions in the popup.
const MAX_VISIBLE: i32 = 8;

/// Skill suggestion popup widget.
///
/// Renders a dropdown list of skill suggestions above the input area.
pub struct SkillSuggestionPopup<'a> {
    state: &'a SkillSuggestionState,
}

impl<'a> SkillSuggestionPopup<'a> {
    /// Create a new skill suggestion popup.
    pub fn new(state: &'a SkillSuggestionState) -> Self {
        Self { state }
    }

    /// Calculate the area for the popup based on input position.
    ///
    /// The popup appears above the input widget, anchored to the left,
    /// with enough width to show skill names and descriptions.
    pub fn calculate_area(&self, input_area: Rect, terminal_height: u16) -> Rect {
        let suggestion_count = self.state.suggestions.len() as i32;
        let visible_count = suggestion_count.min(MAX_VISIBLE);

        // Height: suggestions + 2 for border + 1 for hint line
        let height = (visible_count as u16 + 3).min(terminal_height / 3);

        // Width: Use most of the input area width
        let width = input_area.width.min(60).max(30);

        // Position: above input area
        let x = input_area.x;
        let y = input_area.y.saturating_sub(height);

        // Ensure we don't go off-screen
        let y = if y + height > terminal_height {
            terminal_height.saturating_sub(height)
        } else {
            y
        };

        Rect::new(x, y, width, height)
    }
}

impl Widget for SkillSuggestionPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        // Clear the popup area
        Clear.render(area, buf);

        // Create border with query in title
        let title = format!(" /{} ", self.state.query);
        let block = Block::default()
            .title(title.bold())
            .borders(Borders::ALL)
            .border_style(Style::default().magenta());

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
                Style::default().bg(Color::Magenta).fg(Color::White)
            } else {
                Style::default()
            };

            // Clear line with background
            buf.set_style(item_area, style);

            // Build the display text: "/name - description"
            let prefix = if is_selected { "▸ " } else { "  " };
            let name_width = suggestion.name.len().min(20);
            let desc_start = name_width + 4; // "  /name - "

            // Render prefix
            buf.set_string(inner.x, y, prefix, style);

            // Render skill name with highlighting
            let name_x = inner.x + 2;
            if is_selected {
                // When selected, just render without highlight (selection bg is enough)
                buf.set_string(name_x, y, format!("/{}", suggestion.name), style);
            } else {
                // Apply match highlighting
                buf.set_string(name_x, y, "/", style);
                let mut x = name_x + 1;
                for (char_idx, c) in suggestion.name.chars().enumerate() {
                    let is_match = suggestion.match_indices.contains(&char_idx);
                    let char_style = if is_match {
                        style.bold().magenta()
                    } else {
                        style
                    };
                    buf.set_string(x, y, c.to_string(), char_style);
                    x += 1;
                }
            }

            // Render description (truncated if needed)
            let desc_x = inner.x + desc_start as u16;
            if desc_x < inner.x + inner.width - 3 {
                let available_width = (inner.x + inner.width - desc_x - 1) as usize;
                let desc = if suggestion.description.len() > available_width {
                    format!(
                        " - {}...",
                        &suggestion.description[..available_width.saturating_sub(4)]
                    )
                } else {
                    format!(" - {}", suggestion.description)
                };
                buf.set_string(desc_x, y, desc, style.dim());
            }

            y += 1;
        }

        // Render hint line at bottom
        if inner.height > 1 {
            let hint_y = inner.y + inner.height - 1;
            let hint = if self.state.loading {
                "Loading..."
            } else if self.state.suggestions.is_empty() {
                "No matching skills"
            } else {
                "Tab/Enter: Select  Esc: Dismiss"
            };
            buf.set_string(inner.x, hint_y, hint, Style::default().dim());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SkillSuggestionItem;

    fn create_test_state() -> SkillSuggestionState {
        let mut state = SkillSuggestionState::new("com".to_string(), 0);
        state.update_suggestions(vec![
            SkillSuggestionItem {
                name: "commit".to_string(),
                description: "Generate a commit message".to_string(),
                score: -100,
                match_indices: vec![0, 1, 2],
            },
            SkillSuggestionItem {
                name: "config".to_string(),
                description: "Configure settings".to_string(),
                score: -98,
                match_indices: vec![0, 1],
            },
        ]);
        state
    }

    #[test]
    fn test_popup_creation() {
        let state = create_test_state();
        let popup = SkillSuggestionPopup::new(&state);

        let input_area = Rect::new(0, 20, 80, 3);
        let area = popup.calculate_area(input_area, 24);

        assert!(area.width >= 30);
        assert!(area.height >= 3);
    }

    #[test]
    fn test_popup_render() {
        let state = create_test_state();
        let popup = SkillSuggestionPopup::new(&state);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        popup.render(area, &mut buf);

        // Should contain the query
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("com"));
    }

    #[test]
    fn test_empty_suggestions() {
        let mut state = SkillSuggestionState::new("xyz".to_string(), 0);
        state.update_suggestions(vec![]);
        let popup = SkillSuggestionPopup::new(&state);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        popup.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("No matching"));
    }
}
