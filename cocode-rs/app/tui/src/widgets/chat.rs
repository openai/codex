//! Chat history widget.
//!
//! Displays the conversation messages with support for:
//! - Message role indicators (user/assistant)
//! - Streaming content
//! - Thinking content (collapsed by default)
//! - Animated thinking block with duration
//! - Scroll position

use std::time::Duration;

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

use crate::i18n::t;
use crate::state::ChatMessage;
use crate::state::MessageRole;

/// Chat history widget.
pub struct ChatWidget<'a> {
    messages: &'a [ChatMessage],
    scroll_offset: i32,
    streaming_content: Option<&'a str>,
    streaming_thinking: Option<&'a str>,
    show_thinking: bool,
    /// Whether currently thinking (for animation).
    is_thinking: bool,
    /// Current animation frame (0-7).
    animation_frame: u8,
    /// Duration of current or last thinking phase.
    thinking_duration: Option<Duration>,
}

impl<'a> ChatWidget<'a> {
    /// Create a new chat widget.
    pub fn new(messages: &'a [ChatMessage]) -> Self {
        Self {
            messages,
            scroll_offset: 0,
            streaming_content: None,
            streaming_thinking: None,
            show_thinking: false,
            is_thinking: false,
            animation_frame: 0,
            thinking_duration: None,
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

    /// Set the streaming thinking content.
    pub fn streaming_thinking(mut self, thinking: Option<&'a str>) -> Self {
        self.streaming_thinking = thinking;
        self
    }

    /// Set whether to show thinking content.
    pub fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    /// Set whether currently thinking (for animation).
    pub fn is_thinking(mut self, thinking: bool) -> Self {
        self.is_thinking = thinking;
        self
    }

    /// Set the animation frame.
    pub fn animation_frame(mut self, frame: u8) -> Self {
        self.animation_frame = frame;
        self
    }

    /// Set the thinking duration.
    pub fn thinking_duration(mut self, duration: Option<Duration>) -> Self {
        self.thinking_duration = duration;
        self
    }

    /// Format duration for display (e.g., "2.3s").
    fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs_f64();
        if secs < 1.0 {
            format!("{:.0}ms", secs * 1000.0)
        } else if secs < 60.0 {
            format!("{:.1}s", secs)
        } else {
            let mins = secs / 60.0;
            format!("{:.1}m", mins)
        }
    }

    /// Get the animation character for the thinking indicator.
    fn thinking_animation_char(&self) -> char {
        const SPINNER: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER[self.animation_frame as usize % SPINNER.len()]
    }

    /// Format a message for display.
    fn format_message(&self, message: &ChatMessage) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Role indicator
        let role_span = match message.role {
            MessageRole::User => Span::raw(format!("▶ {}", t!("chat.you"))).bold().cyan(),
            MessageRole::Assistant => Span::raw(format!("◀ {}", t!("chat.assistant")))
                .bold()
                .green(),
            MessageRole::System => Span::raw(format!("⚙ {}", t!("chat.system")))
                .bold()
                .yellow(),
        };
        lines.push(Line::from(role_span));

        // Thinking content (if any)
        if let Some(ref thinking) = message.thinking {
            if !thinking.is_empty() {
                let word_count = thinking.split_whitespace().count();
                let tokens = (word_count as f64 * 1.3) as i32;

                if self.show_thinking {
                    // Show expanded thinking content with styled header
                    let header = format!("  💭 {}", t!("chat.thinking_tokens", tokens = tokens));
                    lines.push(Line::from(Span::raw(header).dim().italic().magenta()));
                    for line in thinking.lines() {
                        lines.push(Line::from(Span::raw(format!("    {line}")).dim()));
                    }
                    lines.push(Line::from("")); // Separator
                } else {
                    // Show collapsed indicator
                    let indicator =
                        format!("  ▸ {}", t!("chat.thinking_collapsed", tokens = tokens));
                    lines.push(Line::from(Span::raw(indicator).dim().magenta()));
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
        let has_streaming = self.streaming_content.is_some()
            || self
                .streaming_thinking
                .as_ref()
                .is_some_and(|t| !t.is_empty());

        if has_streaming {
            all_lines.push(Line::from(
                Span::raw(format!("◀ {}", t!("chat.assistant")))
                    .bold()
                    .green(),
            ));

            // Show thinking content (collapsed indicator or expanded)
            if let Some(thinking) = self.streaming_thinking {
                if !thinking.is_empty() {
                    // Build duration string
                    let duration_str = self
                        .thinking_duration
                        .map(Self::format_duration)
                        .unwrap_or_default();

                    if self.show_thinking {
                        // Show expanded thinking content with animated header
                        let header = if self.is_thinking {
                            let spinner = self.thinking_animation_char();
                            format!(
                                "  {spinner} {}",
                                t!("chat.thinking_active", duration = duration_str)
                            )
                        } else {
                            format!(
                                "  💭 {}",
                                t!("chat.thinking_active", duration = duration_str)
                            )
                        };
                        all_lines.push(Line::from(Span::raw(header).dim().italic().magenta()));

                        for line in thinking.lines() {
                            all_lines.push(Line::from(Span::raw(format!("    {line}")).dim()));
                        }
                        if self.is_thinking {
                            all_lines.push(Line::from(Span::raw("    ▌").dim().slow_blink()));
                        }
                    } else {
                        // Show collapsed indicator with word count estimate and animation
                        let word_count = thinking.split_whitespace().count();
                        let tokens = (word_count as f64 * 1.3) as i32;
                        let indicator = if self.is_thinking {
                            let spinner = self.thinking_animation_char();
                            format!(
                                "  {spinner} {}",
                                t!(
                                    "chat.thinking_active_collapsed",
                                    tokens = tokens,
                                    duration = duration_str
                                )
                            )
                        } else {
                            format!(
                                "  ▸ {}",
                                t!(
                                    "chat.thinking_active_collapsed",
                                    tokens = tokens,
                                    duration = duration_str
                                )
                            )
                        };
                        all_lines.push(Line::from(Span::raw(indicator).dim().magenta()));
                    }
                }
            }

            // Show main streaming content
            if let Some(content) = self.streaming_content {
                if !content.is_empty() {
                    for line in content.lines() {
                        all_lines.push(Line::from(Span::raw(format!("  {line}"))));
                    }
                }
                all_lines.push(Line::from(Span::raw("  ▌").slow_blink()));
            }
        }

        // Create the block
        let block = Block::default().borders(Borders::NONE).title_bottom(
            format!(
                " {} ",
                t!("chat.messages_count", count = self.messages.len())
            )
            .dim(),
        );

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

    #[test]
    fn test_format_duration() {
        assert_eq!(
            ChatWidget::format_duration(Duration::from_millis(500)),
            "500ms"
        );
        assert_eq!(ChatWidget::format_duration(Duration::from_secs(2)), "2.0s");
        assert_eq!(ChatWidget::format_duration(Duration::from_secs(90)), "1.5m");
    }

    #[test]
    fn test_thinking_animation_char() {
        let widget = ChatWidget::new(&[]);
        let char0 = widget.thinking_animation_char();
        assert!(!char0.is_ascii()); // Should be a Unicode spinner char

        let widget = ChatWidget::new(&[]).animation_frame(4);
        let char4 = widget.thinking_animation_char();
        assert_ne!(char0, char4); // Different frames have different chars
    }
}
