use std::path::PathBuf;

use codex_core::config::set_project_trusted;
use codex_core::git_info::resolve_root_git_project_for_trust;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;

pub(crate) struct TrustDirectoryWidget {
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub is_git_repo: bool,
    pub selection: Option<TrustDirectorySelection>,
    pub highlighted: TrustDirectorySelection,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustDirectorySelection {
    Trust,
    DontTrust,
}

impl WidgetRef for &TrustDirectoryWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                "> ".into(),
                "You are running Codex in ".bold(),
                self.cwd.to_string_lossy().to_string().into(),
            ]),
            "".into(),
        ];

        if self.is_git_repo {
            lines.push(
                "  Since this folder is version controlled, you may wish to allow Codex".into(),
            );
            lines.push("  to work in this folder without asking for approval.".into());
        } else {
            lines.push(
                "  Since this folder is not version controlled, we recommend requiring".into(),
            );
            lines.push("  approval of all edits and commands.".into());
        }
        lines.push("".into());

        let create_option =
            |idx: usize, option: TrustDirectorySelection, text: &str| -> Line<'static> {
                let is_selected = self.highlighted == option;
                if is_selected {
                    Line::from(format!("> {}. {text}", idx + 1)).cyan()
                } else {
                    Line::from(format!("  {}. {}", idx + 1, text))
                }
            };

        if self.is_git_repo {
            lines.push(create_option(
                0,
                TrustDirectorySelection::Trust,
                "Yes, allow Codex to work in this folder without asking for approval",
            ));
            lines.push(create_option(
                1,
                TrustDirectorySelection::DontTrust,
                "No, ask me to approve edits and commands",
            ));
        } else {
            lines.push(create_option(
                0,
                TrustDirectorySelection::Trust,
                "Allow Codex to work in this folder without asking for approval",
            ));
            lines.push(create_option(
                1,
                TrustDirectorySelection::DontTrust,
                "Require approval of edits and commands",
            ));
        }
        lines.push("".into());
        if let Some(error) = &self.error {
            lines.push(Line::from(format!("  {error}")).fg(Color::Red));
            lines.push("".into());
        }
        // AE: Following styles.md, this should probably be Cyan because it's a user input tip.
        //     But leaving this for a future cleanup.
        lines.push(Line::from("  Press Enter to continue").add_modifier(Modifier::DIM));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl KeyboardHandler for TrustDirectoryWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlighted = TrustDirectorySelection::Trust;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlighted = TrustDirectorySelection::DontTrust;
            }
            KeyCode::Char('1') => self.handle_trust(),
            KeyCode::Char('2') => self.handle_dont_trust(),
            KeyCode::Enter => match self.highlighted {
                TrustDirectorySelection::Trust => self.handle_trust(),
                TrustDirectorySelection::DontTrust => self.handle_dont_trust(),
            },
            _ => {}
        }
    }
}

impl StepStateProvider for TrustDirectoryWidget {
    fn get_step_state(&self) -> StepState {
        match self.selection {
            Some(_) => StepState::Complete,
            None => StepState::InProgress,
        }
    }
}

impl TrustDirectoryWidget {
    fn handle_trust(&mut self) {
        let target =
            resolve_root_git_project_for_trust(&self.cwd).unwrap_or_else(|| self.cwd.clone());
        if let Err(e) = set_project_trusted(&self.codex_home, &target) {
            tracing::error!("Failed to set project trusted: {e:?}");
            self.error = Some(format!("Failed to set trust for {}: {e}", target.display()));
        }

        self.selection = Some(TrustDirectorySelection::Trust);
    }

    fn handle_dont_trust(&mut self) {
        self.highlighted = TrustDirectorySelection::DontTrust;
        self.selection = Some(TrustDirectorySelection::DontTrust);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn release_event_does_not_change_selection() {
        let mut widget = TrustDirectoryWidget {
            codex_home: PathBuf::from("."),
            cwd: PathBuf::from("."),
            is_git_repo: false,
            selection: None,
            highlighted: TrustDirectorySelection::DontTrust,
            error: None,
        };

        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        };
        widget.handle_key_event(release);
        assert_eq!(widget.selection, None);

        let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        widget.handle_key_event(press);
        assert_eq!(widget.selection, Some(TrustDirectorySelection::DontTrust));
    }
}
