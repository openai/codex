//! Full‑screen warning displayed when the user selects the fully‑unsafe
//! execution preset (Full yolo). This screen blocks input until the user
//! explicitly confirms or cancels the action.

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

const DANGER_TEXT: &str = "You're about to disable both approvals and sandboxing.\n\
This gives the agent full, unrestricted access to your system.\n\
\n\
The agent can and will do stupid and destructive things as your user. Only proceed if you fully understand the risks.";

pub(crate) enum DangerWarningOutcome {
    Continue,
    Cancel,
    None,
}

pub(crate) struct DangerWarningScreen;

impl DangerWarningScreen {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn handle_key_event(&self, key_event: KeyEvent) -> DangerWarningOutcome {
        match key_event.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => DangerWarningOutcome::Continue,
            KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => DangerWarningOutcome::Cancel,
            _ => DangerWarningOutcome::None,
        }
    }
}

impl WidgetRef for &DangerWarningScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        const MIN_WIDTH: u16 = 45;
        const MIN_HEIGHT: u16 = 15;
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let p = Paragraph::new(DANGER_TEXT)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red));
            p.render(area, buf);
            return;
        }

        let popup_width = std::cmp::max(MIN_WIDTH, (area.width as f32 * 0.6) as u16);
        let popup_height = std::cmp::max(MIN_HEIGHT, (area.height as f32 * 0.3) as u16);
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .title(Span::styled(
                "Danger: Full system access",
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
            ));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(inner);

        let text_block = Block::default().borders(Borders::ALL);
        let text_inner = text_block.inner(chunks[0]);
        text_block.render(chunks[0], buf);

        let p = Paragraph::new(DANGER_TEXT)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left)
            .style(Style::default().fg(Color::Red));
        p.render(text_inner, buf);

        let action_block = Block::default().borders(Borders::ALL);
        let action_inner = action_block.inner(chunks[1]);
        action_block.render(chunks[1], buf);

        let action_text = Paragraph::new("press 'y' to proceed, 'n' to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().add_modifier(Modifier::BOLD));
        action_text.render(action_inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn keys_map_to_expected_outcomes() {
        let screen = DangerWarningScreen::new();
        // Continue confirmations
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            DangerWarningOutcome::Continue
        ));
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)),
            DangerWarningOutcome::Continue
        ));

        // Cancellations
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            DangerWarningOutcome::Cancel
        ));
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            DangerWarningOutcome::Cancel
        ));
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            DangerWarningOutcome::Cancel
        ));

        // Irrelevant key is ignored
        assert!(matches!(
            screen.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            DangerWarningOutcome::None
        ));
    }
}
