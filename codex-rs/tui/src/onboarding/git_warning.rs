use std::path::PathBuf;

use codex_core::util::is_inside_git_repo;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::app_event_sender::AppEventSender;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;

pub(crate) struct GitWarningWidget {
    pub event_tx: AppEventSender,
    pub cwd: PathBuf,
    pub selection: Option<GitWarningSelection>,
}

pub(crate) enum GitWarningSelection {
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

impl KeyboardHandler for GitWarningWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                self.selection = Some(GitWarningSelection::Cancelled);
            }
            KeyCode::Enter => {
                self.selection = Some(GitWarningSelection::Cancelled);
            }
            _ => {}
        }
    }
}

impl StepStateProvider for GitWarningWidget {
    fn get_step_state(&self) -> StepState {
        let is_git_repo = is_inside_git_repo(&self.cwd);
        match is_git_repo {
            true => StepState::Hidden,
            false => match self.selection {
                Some(_) => StepState::Complete,
                None => StepState::InProgress,
            },
        }
    }
}
