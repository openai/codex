use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::colors::LIGHT_BLUE;
use crate::onboarding::onboarding_screen::KeyEventResult;
use crate::onboarding::onboarding_screen::KeyboardHandler;

pub(crate) struct SecurityNotesState {}

impl KeyboardHandler for SecurityNotesState {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult {
        match key_event.code {
            KeyCode::Enter => KeyEventResult::Continue,
            KeyCode::Esc | KeyCode::Char('q') => KeyEventResult::Quit,
            _ => KeyEventResult::None,
        }
    }
}

pub(crate) struct SecurityNotesWidget;

impl WidgetRef for &SecurityNotesWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = vec![
            Line::from(vec![
                Span::raw("> "),
                Span::styled(
                    "Security notes:",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Codex can make mistakes",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("Check important info."),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Due to prompt injection risks, only use it with code you trust",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("For more details see https://github.com/openai/codex"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "You're in control",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("ChatGPT always respects your training data preferences."),
            Line::from(""),
            Line::from("Press Enter to continue").style(Style::default().fg(LIGHT_BLUE)),
        ];

        Paragraph::new(lines).render(area, buf);
    }
}

impl KeyboardHandler for SecurityNotesWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult {
        match key_event.code {
            KeyCode::Enter => KeyEventResult::Continue,
            KeyCode::Esc | KeyCode::Char('q') => KeyEventResult::Quit,
            _ => KeyEventResult::None,
        }
    }
}
