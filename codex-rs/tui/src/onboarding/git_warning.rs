use std::path::PathBuf;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent::RequestRedraw;
use crate::app_event_sender::AppEventSender;
use crate::onboarding::onboarding_screen::KeyEventResult;
use crate::onboarding::onboarding_screen::KeyboardHandler;

pub(crate) struct GitWarningWidget {
    pub event_tx: AppEventSender,
    pub cwd: PathBuf,
    pub selection: Option<Selection>,
}

pub(crate) enum Selection {
    Confirmed,
    Cancelled,
}

impl WidgetRef for &GitWarningWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(format!(
                "> You are running Codex in {}. Since this folder is not version controlled,",
                self.cwd.to_string_lossy().to_string(),
            )),
            Line::from(format!(
                "  we recommend careful review of all edits and commands before they are run."
            )),
        ];
        Paragraph::new(lines).render(area, buf);
    }
}

impl KeyboardHandler for &GitWarningWidget {
    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) -> KeyEventResult {
        match key_event.code {
            KeyCode::Esc => {
                self.selection = Selection::Cancelled;
                self.event_tx.send(RequestRedraw);
            }
            KeyCode::Enter => {
                self.selection = 
            }
            _ => {}
        }
    }
}
