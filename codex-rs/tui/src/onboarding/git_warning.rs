use std::path::PathBuf;

use codex_core::util::is_inside_git_repo;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::colors::LIGHT_BLUE;

use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;

pub(crate) struct GitWarningWidget {
    pub event_tx: AppEventSender,
    pub cwd: PathBuf,
    pub selection: Option<GitWarningSelection>,
    pub highlighted: GitWarningSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitWarningSelection {
    Continue,
    Exit,
}

impl WidgetRef for &GitWarningWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::raw("> "),
                Span::raw("You are running Codex in "),
                Span::styled(
                    self.cwd.to_string_lossy().to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(". This folder is not version controlled."),
            ]),
            Line::from(""),
            Line::from("  Do you want to continue?"),
            Line::from(""),
        ];

        let create_option =
            |idx: usize, option: GitWarningSelection, text: &str| -> Line<'static> {
                let is_selected = self.highlighted == option;
                if is_selected {
                    Line::from(vec![
                        Span::styled(
                            format!("> {}. ", idx + 1),
                            Style::default().fg(LIGHT_BLUE).add_modifier(Modifier::DIM),
                        ),
                        Span::styled(text.to_owned(), Style::default().fg(LIGHT_BLUE)),
                    ])
                } else {
                    Line::from(format!("  {}. {}", idx + 1, text))
                }
            };

        lines.push(create_option(0, GitWarningSelection::Continue, "Yes"));
        lines.push(create_option(1, GitWarningSelection::Exit, "No"));
        lines.push(Line::from(""));
        lines.push(Line::from("  Press Enter to continue").add_modifier(Modifier::DIM));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl KeyboardHandler for GitWarningWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlighted = GitWarningSelection::Continue;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlighted = GitWarningSelection::Exit;
            }
            KeyCode::Char('1') => self.handle_continue(),
            KeyCode::Char('2') => self.handle_quit(),
            KeyCode::Enter => match self.highlighted {
                GitWarningSelection::Continue => self.handle_continue(),
                GitWarningSelection::Exit => self.handle_quit(),
            },
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

impl GitWarningWidget {
    fn handle_continue(&mut self) {
        self.selection = Some(GitWarningSelection::Continue);
    }

    fn handle_quit(&mut self) {
        self.highlighted = GitWarningSelection::Exit;
        self.event_tx.send(AppEvent::ExitRequest);
    }
}
