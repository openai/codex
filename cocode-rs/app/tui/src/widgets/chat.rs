//! Chat history widget.
//!
//! Displays the conversation messages with support for:
//! - Message role indicators (user/assistant)
//! - Streaming content
//! - Thinking content (collapsed by default)
//! - Scroll position

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

use crate::state::ChatMessage;
use crate::state::MessageRole;

/// Chat history widget.
pub struct ChatWidget<'a> {
    messages: &'a [ChatMessage],
    scroll_offset: i32,
    streaming_content: Option<&'a str>,
    show_thinking: bool,
}

impl<'a> ChatWidget<'a> {
    /// Create a new chat widget.
    pub fn new(messages: &'a [ChatMessage]) -> Self {
        Self {
            messages,
            scroll_offset: 0,
            streaming_content: None,
            show_thinking: false,
        }
    }

    /// Set the scroll offset.
    pub fn scroll(mut self, offset: i32) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Set the streaming content.
    pub fn streaming(mut self, content: Option<&'a str>) -> Self {
        self.streaming_content = content;
        self
    }

    /// Set whether to show thinking content.
    pub fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    /// Format a message for display.
    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Role indicator
        let role_span = match message.role {
            MessageRole::User => Span::raw("▶ You").bold().cyan(),
            MessageRole::Assistant => Span::raw("◀ Assistant").bold().green(),
            MessageRole::System => Span::raw("⚙ System").bold().yellow(),
        };
        lines.push(Line::from(role_span));

        // Thinking content (if any and showing)
        if self.show_thinking {
            if let Some(ref thinking) = message.thinking {
                if !thinking.is_empty() {
                    lines.push(Line::from(Span::raw("  💭 Thinking:").dim().italic()));
                    for line in thinking.lines() {
                        lines.push(Line::from(Span::raw(format!("    {line}")).dim()));
                    }
                    lines.push(Line::from("")); // Separator
                }
            }
        }

        // Message content
        for line in message.content.lines() {
            lines.push(Line::from(Span::raw(format!("  {line}"))));
        }

        // Streaming indicator
        if message.streaming {
            lines.push(Line::from(Span::raw("  ▌").slow_blink()));
        }

        // Empty line after message
        lines.push(Line::from(""));

        lines
    }
}

impl Widget for ChatWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 {
            return;
        }

        // Build all lines
        let mut all_lines: Vec<Line> = Vec::new();

        for message in self.messages {
            all_lines.extend(self.format_message(message));
        }

        // Add streaming content if present
        if let Some(content) = self.streaming_content {
            all_lines.push(Line::from(Span::raw("◀ Assistant").bold().green()));
            for line in content.lines() {
                all_lines.push(Line::from(Span::raw(format!("  {line}"))));
            }
            all_lines.push(Line::from(Span::raw("  ▌").slow_blink()));
        }

        // Create the block
        let block = Block::default()
            .borders(Borders::NONE)
            .title_bottom(format!(" {} messages ", self.messages.len()).dim());

        // Calculate scroll
        let total_lines = all_lines.len();
        let visible_lines = (area.height - 2) as usize; // Account for borders
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let scroll = (self.scroll_offset as usize).min(max_scroll);

        // Create paragraph with scroll
        let paragraph = Paragraph::new(all_lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_widget_empty() {
        let messages: Vec<ChatMessage> = vec![];
        let widget = ChatWidget::new(&messages);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without panic
    }

    #[test]
    fn test_chat_widget_with_messages() {
        let messages = vec![
            ChatMessage::user("1", "Hello"),
            ChatMessage::assistant("2", "Hi there!"),
        ];
        let widget = ChatWidget::new(&messages);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("You"));
        assert!(content.contains("Hello"));
        assert!(content.contains("Assistant"));
    }

    #[test]
    fn test_format_message_user() {
        let widget = ChatWidget::new(&[]);
        let msg = ChatMessage::user("1", "Test message");
        let lines = widget.format_message(&msg);

        assert!(!lines.is_empty());
        // First line should be role indicator
        let first_line: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(first_line.contains("You"));
    }

    #[test]
    fn test_format_message_with_thinking() {
        let widget = ChatWidget::new(&[]).show_thinking(true);
        let mut msg = ChatMessage::assistant("1", "Response");
        msg.thinking = Some("I'm thinking about this...".to_string());

        let lines = widget.format_message(&msg);
        let content: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();

        assert!(content.contains("Thinking"));
    }
}
