//! Queued commands list widget.
//!
//! Displays queued commands waiting to be processed. These commands
//! were entered during streaming and will be:
//! 1. Injected as real-time steering into the current turn
//! 2. Executed as new user turns after the agent becomes idle

use cocode_protocol::UserQueuedCommand;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::i18n::t;

/// Widget to render queued commands above the input box.
///
/// Layout (aligns with Claude Code's QueuedCommandsList):
/// ```text
/// 🕐 Waiting:
///   • "use TypeScript instead"
///   • "add error handling"
/// ```
pub struct QueuedListWidget<'a> {
    commands: &'a [UserQueuedCommand],
    max_display: i32,
}

impl<'a> QueuedListWidget<'a> {
    /// Create a new queued list widget.
    pub fn new(commands: &'a [UserQueuedCommand]) -> Self {
        Self {
            commands,
            max_display: 5,
        }
    }

    /// Set the maximum number of commands to display.
    pub fn max_display(mut self, max: i32) -> Self {
        self.max_display = max;
        self
    }

    /// Calculate the height needed to render the queued list.
    ///
    /// Returns 0 if there are no queued commands.
    pub fn required_height(&self) -> u16 {
        if self.commands.is_empty() {
            return 0;
        }

        let count = (self.commands.len() as i32).min(self.max_display) as u16;
        // 1 line for header + 1 line per command
        1 + count
    }
}

impl Widget for QueuedListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.commands.is_empty() || area.height < 2 || area.width < 10 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        // Header line: "🕐 Waiting:" (dimmed)
        let waiting_label = t!("status.waiting").to_string();
        lines.push(Line::from(Span::raw(format!("  {waiting_label}")).dim()));

        // Command lines
        for cmd in self.commands.iter().take(self.max_display as usize) {
            // Truncate the prompt if too long
            let max_len = area.width.saturating_sub(8) as usize; // "    • " prefix + quotes
            let prompt = if cmd.prompt.len() > max_len {
                format!("{}...", &cmd.prompt[..max_len.saturating_sub(3)])
            } else {
                cmd.prompt.clone()
            };

            lines.push(Line::from(vec![
                Span::raw("    ").dim(),
                Span::raw("• ").dim(),
                Span::raw(format!("\"{prompt}\"")).dim(),
            ]));
        }

        // Show "+N more" if there are more commands than displayed
        let remaining = self.commands.len() as i32 - self.max_display;
        if remaining > 0 {
            lines.push(Line::from(
                Span::raw(format!("    +{remaining} more..."))
                    .dim()
                    .italic(),
            ));
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_command(prompt: &str) -> UserQueuedCommand {
        UserQueuedCommand {
            id: format!("cmd-{}", prompt.len()),
            prompt: prompt.to_string(),
            queued_at: 1234567890,
        }
    }

    #[test]
    fn test_empty_commands() {
        let commands: Vec<UserQueuedCommand> = vec![];
        let widget = QueuedListWidget::new(&commands);
        assert_eq!(widget.required_height(), 0);
    }

    #[test]
    fn test_single_command() {
        let commands = vec![make_command("use TypeScript")];
        let widget = QueuedListWidget::new(&commands);
        assert_eq!(widget.required_height(), 2); // header + 1 command
    }

    #[test]
    fn test_multiple_commands() {
        let commands = vec![
            make_command("use TypeScript"),
            make_command("add error handling"),
            make_command("include tests"),
        ];
        let widget = QueuedListWidget::new(&commands);
        assert_eq!(widget.required_height(), 4); // header + 3 commands
    }

    #[test]
    fn test_max_display_limit() {
        let commands: Vec<_> = (0..10).map(|i| make_command(&format!("cmd {i}"))).collect();
        let widget = QueuedListWidget::new(&commands).max_display(3);
        assert_eq!(widget.required_height(), 4); // header + 3 commands (limited)
    }

    #[test]
    fn test_render_empty() {
        let commands: Vec<UserQueuedCommand> = vec![];
        let widget = QueuedListWidget::new(&commands);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without panic, and buffer should be empty
    }

    #[test]
    fn test_render_with_commands() {
        let commands = vec![
            make_command("use TypeScript"),
            make_command("add error handling"),
        ];
        let widget = QueuedListWidget::new(&commands);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without panic
    }
}
