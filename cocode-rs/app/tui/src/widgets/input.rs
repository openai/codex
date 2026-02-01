//! Input widget.
//!
//! Multi-line input field with cursor support.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use crate::state::InputState;

/// Input widget for user text entry.
pub struct InputWidget<'a> {
    input: &'a InputState,
    focused: bool,
    placeholder: Option<&'a str>,
}

impl<'a> InputWidget<'a> {
    /// Create a new input widget.
    pub fn new(input: &'a InputState) -> Self {
        Self {
            input,
            focused: true,
            placeholder: None,
        }
    }

    /// Set whether the input is focused.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = Some(text);
        self
    }

    /// Get the display lines with cursor.
    fn get_lines(&self) -> Vec<Line<'static>> {
        let text = self.input.text();

        // Show placeholder if empty
        if text.is_empty() {
            if let Some(placeholder) = self.placeholder {
                return vec![Line::from(
                    Span::raw(placeholder.to_string()).dim().italic(),
                )];
            }
            // Just show cursor
            if self.focused {
                return vec![Line::from(Span::raw("▌").slow_blink())];
            }
            return vec![Line::from("")];
        }

        // Build lines with cursor
        let cursor_pos = self.input.cursor as usize;
        let mut lines = Vec::new();

        for (line_idx, line) in text.lines().enumerate() {
            // Calculate if cursor is on this line
            let line_start = text
                .lines()
                .take(line_idx)
                .map(|l| l.len() + 1) // +1 for newline
                .sum::<usize>();
            let line_end = line_start + line.len();

            if self.focused && cursor_pos >= line_start && cursor_pos <= line_end {
                // Cursor is on this line
                let cursor_in_line = cursor_pos - line_start;
                let before = &line[..cursor_in_line.min(line.len())];
                let after = &line[cursor_in_line.min(line.len())..];

                let mut spans = vec![Span::raw(before.to_string())];
                spans.push(Span::raw("▌").slow_blink());
                if !after.is_empty() {
                    spans.push(Span::raw(after.to_string()));
                }
                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(Span::raw(line.to_string())));
            }
        }

        // Handle cursor at end of text (after last line)
        if self.focused && cursor_pos >= text.len() {
            if let Some(last) = lines.last_mut() {
                last.spans.push(Span::raw("▌").slow_blink());
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::raw("▌").slow_blink()));
        }

        lines
    }
}

impl Widget for InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let lines = self.get_lines();

        // Create block
        let border_style = if self.focused {
            ratatui::style::Style::default().cyan()
        } else {
            ratatui::style::Style::default().dim()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Message ")
            .title_style(if self.focused {
                ratatui::style::Style::default().bold()
            } else {
                ratatui::style::Style::default().dim()
            });

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_widget_empty() {
        let input = InputState::default();
        let widget = InputWidget::new(&input);

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without panic
    }

    #[test]
    fn test_input_widget_with_text() {
        let mut input = InputState::default();
        input.set_text("Hello");
        let widget = InputWidget::new(&input);

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Hello"));
    }

    #[test]
    fn test_input_widget_placeholder() {
        let input = InputState::default();
        let widget = InputWidget::new(&input).placeholder("Type a message...");

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Type a message"));
    }

    #[test]
    fn test_input_widget_unfocused() {
        let input = InputState::default();
        let widget = InputWidget::new(&input).focused(false);

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without cursor blinking
    }

    #[test]
    fn test_get_lines_with_cursor() {
        let mut input = InputState::default();
        input.set_text("Hello");
        input.cursor = 2; // After "He"

        let widget = InputWidget::new(&input);
        let lines = widget.get_lines();

        assert!(!lines.is_empty());
        // Should have cursor in the middle
        let spans: Vec<_> = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(spans.contains(&"He"));
    }
}
