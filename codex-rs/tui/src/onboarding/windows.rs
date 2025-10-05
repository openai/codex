use std::path::PathBuf;

use codex_core::config::set_windows_wsl_setup_acknowledged;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;

pub(crate) const WSL_INSTRUCTIONS: &str = r"Windows Subsystem for Linux (WSL2) is required to run Codex.

To install WSL2:
  1. Open PowerShell as Administrator and run: wsl --install
  2. Restart your machine if prompted.
  3. Launch the Ubuntu shortcut from the Start menu to complete setup.

After installation, reopen Codex from a WSL shell.";

pub(crate) struct WindowsSetupWidget {
    pub codex_home: PathBuf,
    pub selection: Option<WindowsSetupSelection>,
    pub highlighted: WindowsSetupSelection,
    pub error: Option<String>,
    exit_requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSetupSelection {
    Continue,
    Install,
}

impl WindowsSetupWidget {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            selection: None,
            highlighted: WindowsSetupSelection::Continue,
            error: None,
            exit_requested: false,
        }
    }

    fn handle_continue(&mut self) {
        self.highlighted = WindowsSetupSelection::Continue;
        match set_windows_wsl_setup_acknowledged(&self.codex_home, true) {
            Ok(()) => {
                self.selection = Some(WindowsSetupSelection::Continue);
                self.exit_requested = false;
                self.error = None;
            }
            Err(err) => {
                tracing::error!("Failed to persist Windows onboarding acknowledgement: {err:?}");
                self.error = Some(format!("Failed to update config: {err}"));
                self.selection = None;
            }
        }
    }

    fn handle_install(&mut self) {
        self.highlighted = WindowsSetupSelection::Install;
        self.selection = Some(WindowsSetupSelection::Install);
        self.exit_requested = true;
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }
}

impl WidgetRef for &WindowsSetupWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec!["> ".into(), "Codex Windows Support".bold()]),
            Line::from(""),
            Line::from(
                "  Codex support for Windows is in progress. Full support for Codex on Windows requires Windows Subsystem for Linux (WSL2).",
            ),
            Line::from(""),
        ];

        let create_option =
            |idx: usize, option: WindowsSetupSelection, text: &str| -> Line<'static> {
                if self.highlighted == option {
                    Line::from(format!("> {}. {text}", idx + 1)).cyan()
                } else {
                    Line::from(format!("  {}. {}", idx + 1, text))
                }
            };

        lines.push(create_option(
            0,
            WindowsSetupSelection::Continue,
            "Continue anyway",
        ));
        lines.push(create_option(
            1,
            WindowsSetupSelection::Install,
            "Exit and install Windows Subsystem for Linux (WSL2)",
        ));
        lines.push("".into());

        if let Some(error) = &self.error {
            lines.push(Line::from(format!("  {error}")).fg(Color::Red));
            lines.push("".into());
        }

        lines.push(Line::from(vec!["  Press Enter to continue".dim()]));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl KeyboardHandler for WindowsSetupWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlighted = WindowsSetupSelection::Continue;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlighted = WindowsSetupSelection::Install;
            }
            KeyCode::Char('1') => self.handle_continue(),
            KeyCode::Char('2') => self.handle_install(),
            KeyCode::Enter => match self.highlighted {
                WindowsSetupSelection::Continue => self.handle_continue(),
                WindowsSetupSelection::Install => self.handle_install(),
            },
            _ => {}
        }
    }
}

impl StepStateProvider for WindowsSetupWidget {
    fn get_step_state(&self) -> StepState {
        match self.selection {
            Some(WindowsSetupSelection::Continue) => StepState::Hidden,
            Some(WindowsSetupSelection::Install) => StepState::Complete,
            None => StepState::InProgress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn windows_step_hidden_after_continue() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut widget = WindowsSetupWidget::new(temp_dir.path().to_path_buf());

        assert_eq!(widget.get_step_state(), StepState::InProgress);

        widget.handle_continue();

        assert_eq!(widget.get_step_state(), StepState::Hidden);
        assert!(!widget.exit_requested());
    }

    #[test]
    fn windows_step_complete_after_install_selection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut widget = WindowsSetupWidget::new(temp_dir.path().to_path_buf());

        widget.handle_install();

        assert_eq!(widget.get_step_state(), StepState::Complete);
        assert!(widget.exit_requested());
    }
}
