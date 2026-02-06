//! Input widget.
//!
//! Multi-line input field with cursor support and syntax highlighting
//! for @mentions (cyan), /commands (magenta), and paste pills (green).

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
use crate::paste::is_paste_pill;
use crate::state::InputState;

/// Token type for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenType {
    /// Plain text.
    Text,
    /// @mention (file path).
    AtMention,
    /// /command (skill).
    SlashCommand,
    /// Paste pill ([Pasted text #1], [Image #1]).
    PastePill,
}

/// A token in the input text.
#[derive(Debug, Clone)]
struct Token {
    /// Token text.
    text: String,
    /// Token type.
    token_type: TokenType,
}

impl Token {
    fn new(text: impl Into<String>, token_type: TokenType) -> Self {
        Self {
            text: text.into(),
            token_type,
        }
    }
}

/// Tokenize input text for syntax highlighting.
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current_text = String::new();
    let mut chars = text.chars().peekable();
    let mut in_mention = false;
    let mut in_command = false;
    let mut in_pill = false;
    let mut pill_buffer = String::new();

    while let Some(c) = chars.next() {
        match c {
            '[' if !in_mention && !in_command && !in_pill => {
                // Potential start of a paste pill
                // Flush current text
                if !current_text.is_empty() {
                    tokens.push(Token::new(&current_text, TokenType::Text));
                    current_text.clear();
                }
                pill_buffer.push(c);
                in_pill = true;
            }
            ']' if in_pill => {
                // End of potential pill
                pill_buffer.push(c);
                if is_paste_pill(&pill_buffer) {
                    tokens.push(Token::new(&pill_buffer, TokenType::PastePill));
                } else {
                    // Not a valid pill, treat as regular text
                    tokens.push(Token::new(&pill_buffer, TokenType::Text));
                }
                pill_buffer.clear();
                in_pill = false;
            }
            _ if in_pill => {
                pill_buffer.push(c);
                // Safety limit: pills shouldn't be too long
                if pill_buffer.len() > 50 {
                    // Not a pill, flush as text
                    current_text.push_str(&pill_buffer);
                    pill_buffer.clear();
                    in_pill = false;
                }
            }
            '@' if !in_mention && !in_command => {
                // Check if this is a valid @mention start (at start or after whitespace)
                let is_valid_start =
                    current_text.is_empty() || current_text.ends_with(char::is_whitespace);

                if is_valid_start {
                    // Flush current text
                    if !current_text.is_empty() {
                        tokens.push(Token::new(&current_text, TokenType::Text));
                        current_text.clear();
                    }
                    current_text.push(c);
                    in_mention = true;
                } else {
                    current_text.push(c);
                }
            }
            '/' if !in_mention && !in_command => {
                // Check if this is a valid /command start (at start or after whitespace)
                let is_valid_start =
                    current_text.is_empty() || current_text.ends_with(char::is_whitespace);

                if is_valid_start {
                    // Flush current text
                    if !current_text.is_empty() {
                        tokens.push(Token::new(&current_text, TokenType::Text));
                        current_text.clear();
                    }
                    current_text.push(c);
                    in_command = true;
                } else {
                    current_text.push(c);
                }
            }
            ' ' | '\t' | '\n' => {
                // Whitespace ends mentions/commands
                if in_mention {
                    tokens.push(Token::new(&current_text, TokenType::AtMention));
                    current_text.clear();
                    in_mention = false;
                } else if in_command {
                    tokens.push(Token::new(&current_text, TokenType::SlashCommand));
                    current_text.clear();
                    in_command = false;
                }
                current_text.push(c);
            }
            _ => {
                current_text.push(c);
            }
        }
    }

    // Flush any remaining pill buffer as text
    if !pill_buffer.is_empty() {
        current_text.push_str(&pill_buffer);
    }

    // Flush remaining text
    if !current_text.is_empty() {
        let token_type = if in_mention {
            TokenType::AtMention
        } else if in_command {
            TokenType::SlashCommand
        } else {
            TokenType::Text
        };
        tokens.push(Token::new(&current_text, token_type));
    }

    tokens
}

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

    /// Get the display lines with cursor and syntax highlighting.
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

        // Build highlighted spans with cursor
        let cursor_pos = self.input.cursor as usize;
        let tokens = tokenize(text);

        // Build a flat list of styled characters with cursor position
        let mut styled_chars: Vec<(char, TokenType)> = Vec::new();
        for token in &tokens {
            for c in token.text.chars() {
                styled_chars.push((c, token.token_type));
            }
        }

        // Now build lines, inserting cursor at the right position
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut current_line_spans: Vec<Span<'static>> = Vec::new();
        let mut current_span_text = String::new();
        let mut current_token_type: Option<TokenType> = None;
        let mut char_pos = 0_usize;

        for (c, token_type) in &styled_chars {
            // Insert cursor if this is the position
            if self.focused && char_pos == cursor_pos {
                // Flush current span
                if !current_span_text.is_empty() {
                    current_line_spans.push(Self::styled_span(
                        &current_span_text,
                        current_token_type.unwrap_or(TokenType::Text),
                    ));
                    current_span_text.clear();
                }
                current_line_spans.push(Span::raw("▌").slow_blink());
            }

            // Handle newlines
            if *c == '\n' {
                // Flush current span and line
                if !current_span_text.is_empty() {
                    current_line_spans.push(Self::styled_span(
                        &current_span_text,
                        current_token_type.unwrap_or(TokenType::Text),
                    ));
                    current_span_text.clear();
                }
                lines.push(Line::from(current_line_spans));
                current_line_spans = Vec::new();
                current_token_type = None;
            } else {
                // Continue building span
                if current_token_type != Some(*token_type) {
                    // Token type changed, flush current span
                    if !current_span_text.is_empty() {
                        current_line_spans.push(Self::styled_span(
                            &current_span_text,
                            current_token_type.unwrap_or(TokenType::Text),
                        ));
                        current_span_text.clear();
                    }
                    current_token_type = Some(*token_type);
                }
                current_span_text.push(*c);
            }

            char_pos += 1;
        }

        // Flush remaining span
        if !current_span_text.is_empty() {
            current_line_spans.push(Self::styled_span(
                &current_span_text,
                current_token_type.unwrap_or(TokenType::Text),
            ));
        }

        // Insert cursor at end if needed
        if self.focused && char_pos <= cursor_pos {
            current_line_spans.push(Span::raw("▌").slow_blink());
        }

        // Flush remaining line
        if !current_line_spans.is_empty() {
            lines.push(Line::from(current_line_spans));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::raw("▌").slow_blink()));
        }

        lines
    }

    /// Create a styled span based on token type.
    fn styled_span(text: &str, token_type: TokenType) -> Span<'static> {
        match token_type {
            TokenType::Text => Span::raw(text.to_string()),
            TokenType::AtMention => Span::raw(text.to_string()).cyan(),
            TokenType::SlashCommand => Span::raw(text.to_string()).magenta(),
            TokenType::PastePill => Span::raw(text.to_string()).green().italic(),
        }
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
            .title(format!(" {} ", t!("input.title")))
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

    #[test]
    fn test_tokenize_plain_text() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Text);
        assert_eq!(tokens[0].text, "hello world");
    }

    #[test]
    fn test_tokenize_at_mention() {
        let tokens = tokenize("read @src/main.rs please");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, TokenType::Text);
        assert_eq!(tokens[0].text, "read ");
        assert_eq!(tokens[1].token_type, TokenType::AtMention);
        assert_eq!(tokens[1].text, "@src/main.rs");
        assert_eq!(tokens[2].token_type, TokenType::Text);
        assert_eq!(tokens[2].text, " please");
    }

    #[test]
    fn test_tokenize_slash_command() {
        let tokens = tokenize("/commit now");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].token_type, TokenType::SlashCommand);
        assert_eq!(tokens[0].text, "/commit");
        assert_eq!(tokens[1].token_type, TokenType::Text);
        assert_eq!(tokens[1].text, " now");
    }

    #[test]
    fn test_tokenize_mixed() {
        let tokens = tokenize("/review @src/lib.rs");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, TokenType::SlashCommand);
        assert_eq!(tokens[0].text, "/review");
        assert_eq!(tokens[1].token_type, TokenType::Text);
        assert_eq!(tokens[1].text, " ");
        assert_eq!(tokens[2].token_type, TokenType::AtMention);
        assert_eq!(tokens[2].text, "@src/lib.rs");
    }

    #[test]
    fn test_tokenize_at_not_at_start() {
        // @ in middle of word should not be a mention
        let tokens = tokenize("email@example.com");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Text);
    }

    #[test]
    fn test_tokenize_slash_not_at_start() {
        // / in middle of word should not be a command
        let tokens = tokenize("path/to/file");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Text);
    }

    #[test]
    fn test_tokenize_paste_pill() {
        let tokens = tokenize("[Pasted text #1]");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::PastePill);
        assert_eq!(tokens[0].text, "[Pasted text #1]");
    }

    #[test]
    fn test_tokenize_paste_pill_with_lines() {
        let tokens = tokenize("[Pasted text #1 +420 lines]");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::PastePill);
    }

    #[test]
    fn test_tokenize_image_pill() {
        let tokens = tokenize("[Image #1]");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::PastePill);
    }

    #[test]
    fn test_tokenize_mixed_with_pill() {
        let tokens = tokenize("Please analyze [Pasted text #1] and tell me");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, TokenType::Text);
        assert_eq!(tokens[0].text, "Please analyze ");
        assert_eq!(tokens[1].token_type, TokenType::PastePill);
        assert_eq!(tokens[1].text, "[Pasted text #1]");
        assert_eq!(tokens[2].token_type, TokenType::Text);
        assert_eq!(tokens[2].text, " and tell me");
    }

    #[test]
    fn test_tokenize_non_pill_brackets() {
        // Regular brackets that aren't paste pills
        let tokens = tokenize("[some other thing]");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Text);
    }
}
