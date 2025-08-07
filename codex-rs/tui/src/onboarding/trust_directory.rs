use std::path::PathBuf;

use codex_core::config::set_project_trusted;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::colors::LIGHT_BLUE;

use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;
use crate::app::ChatWidgetArgs;
use std::sync::Arc;
use std::sync::Mutex;

pub(crate) struct TrustDirectoryWidget {
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub is_git_repo: bool,
    pub selection: Option<TrustDirectorySelection>,
    pub highlighted: TrustDirectorySelection,
    pub error: Option<String>,
    pub chat_widget_args: Arc<Mutex<ChatWidgetArgs>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrustDirectorySelection {
    Trust,
    DontTrust,
}

impl WidgetRef for &TrustDirectoryWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::raw("> "),
                Span::styled(
                    "You are running Codex in ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(self.cwd.to_string_lossy().to_string()),
            ]),
            Line::from(""),
        ];

        if self.is_git_repo {
            lines.push(Line::from(
                "  Since this folder is version controlled, you may wish to allow Codex",
            ));
            lines.push(Line::from(
                "  to work in this folder without asking for approval.",
            ));
        } else {
            lines.push(Line::from(
                "  Since this folder is not version controlled, we recommend careful review",
            ));
            lines.push(Line::from(
                "  of all edits and commands before they are run.",
            ));
        }
        lines.push(Line::from(""));

        let create_option =
            |idx: usize, option: TrustDirectorySelection, text: &str| -> Line<'static> {
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
                "Yes, ask me to approve edits and commands",
            ));
            lines.push(create_option(
                1,
                TrustDirectorySelection::DontTrust,
                "No, allow Codex to work in this folder without asking",
            ));
        }
        lines.push(Line::from(""));
        if let Some(error) = &self.error {
            lines.push(Line::from(format!("  {error}")).fg(Color::Red));
            lines.push(Line::from(""));
        }
        lines.push(Line::from("  Press Enter to continue").add_modifier(Modifier::DIM));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl KeyboardHandler for TrustDirectoryWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
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
        if let Err(e) = set_project_trusted(&self.codex_home, &self.cwd, true) {
            tracing::error!("Failed to set project trusted: {e:?}");
        }

        // Update the in-memory chat config for this session to a more permissive
        // policy suitable for a trusted workspace.
        if let Ok(mut args) = self.chat_widget_args.lock() {
            args.config.approval_policy = AskForApproval::OnRequest;
            args.config.sandbox_policy = SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![self.cwd.clone()],
                network_access: false,
                include_default_writable_roots: true,
            };
        }

        // TODO: update the config
        self.selection = Some(TrustDirectorySelection::Trust);
    }

    fn handle_dont_trust(&mut self) {
        self.highlighted = TrustDirectorySelection::DontTrust;
        self.selection = Some(TrustDirectorySelection::DontTrust);
    }
}
