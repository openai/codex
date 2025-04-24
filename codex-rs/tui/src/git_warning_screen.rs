//! Full‑screen warning displayed when Codex is started outside a Git
//! repository (unless the user passed `--allow-no-git-exec`). The screen
//! blocks all input until the user explicitly decides whether to continue or
//! quit.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

const NO_GIT_ERROR: &str = "We recommend running codex inside a git repository. \
This helps ensure that changes can be tracked and easily rolled back if necessary. \
Do you wish to proceed?";

/// Result of handling a key event while the warning screen is active.
pub(crate) enum GitWarningOutcome {
    /// User chose to proceed – switch to the main Chat UI.
    Continue,
    /// User opted to quit the application.
    Quit,
    /// No actionable key was pressed – stay on the warning screen.
    None,
}

pub(crate) struct GitWarningScreen;

impl GitWarningScreen {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Handle a key event, returning an outcome indicating whether the user
    /// chose to continue, quit, or neither.
    pub(crate) fn handle_key_event(&self, key_event: KeyEvent) -> GitWarningOutcome {
        match key_event.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => GitWarningOutcome::Continue,
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => GitWarningOutcome::Quit,
            _ => GitWarningOutcome::None,
        }
    }
}

impl WidgetRef for &GitWarningScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        const MIN_WIDTH: u16 = 35;
        const MIN_HEIGHT: u16 = 15;
        // Check if the available area is too small for our popup.
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            // Fallback rendering: a simple abbreviated message that fits the available area.
            let fallback_message = Paragraph::new(NO_GIT_ERROR)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center);
            fallback_message.render(area, buf);
            return;
        }

        // Determine the popup (modal) size – aim for 60 % width, 30 % height
        // but keep a sensible minimum so the content is always readable.
        let popup_width = std::cmp::max(MIN_WIDTH, (area.width as f32 * 0.6) as u16);
        let popup_height = std::cmp::max(MIN_HEIGHT, (area.height as f32 * 0.3) as u16);

        // Center the popup in the available area.
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // The modal block that contains everything.
        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .title(Span::styled(
                "Warning: Not a Git repository", // bold warning title
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
            ));

        // Obtain the inner area before rendering (render consumes the block).
        let inner = popup_block.inner(popup_area);
        popup_block.render(popup_area, buf);

        // Split the inner area vertically into two boxes: one for the warning
        // explanation, one for the user action instructions.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(inner);

        // ----- First box: detailed warning text --------------------------------
        let text_block = Block::default().borders(Borders::ALL);
        let text_inner = text_block.inner(chunks[0]);
        text_block.render(chunks[0], buf);

        let warning_paragraph = Paragraph::new(NO_GIT_ERROR)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);
        warning_paragraph.render(text_inner, buf);

        // ----- Second box: "proceed? y/n" instructions --------------------------
        let action_block = Block::default().borders(Borders::ALL);
        let action_inner = action_block.inner(chunks[1]);
        action_block.render(chunks[1], buf);

        let action_text = Paragraph::new("press 'y' to continue, 'n' to quit")
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        action_text.render(action_inner, buf);
    }
}
