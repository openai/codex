use std::path::PathBuf;

use codex_core::config_edit::ConfigEditsBuilder;
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

pub(crate) const WSL_INSTRUCTIONS: &str = r#"Install WSL2 by opening PowerShell as Administrator and running:
    # Install WSL using the default Linux distribution (Ubuntu).
    # See https://learn.microsoft.com/en-us/windows/wsl/install for more info
    wsl --install

    # Restart your computer, then start a shell inside of Windows Subsystem for Linux
    wsl

    # Install Node.js in WSL via nvm
    # Documentation: https://learn.microsoft.com/en-us/windows/dev-environment/javascript/nodejs-on-wsl
    curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/master/install.sh | bash && export NVM_DIR="$HOME/.nvm" && \. "$NVM_DIR/nvm.sh"
    nvm install 22

    # Install and run Codex in WSL
    npm install --global @openai/codex
    codex

    # Additional details and instructions for how to install and run Codex in WSL:
    https://developers.openai.com/codex/windows"#;

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
            highlighted: WindowsSetupSelection::Install,
            error: None,
            exit_requested: false,
        }
    }

    fn handle_continue(&mut self) {
        self.highlighted = WindowsSetupSelection::Continue;
        match ConfigEditsBuilder::new(&self.codex_home)
            .set_windows_wsl_setup_acknowledged(true)
            .apply_blocking()
        {
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
            Line::from(vec![
                "> ".into(),
                "To use all Codex features, we recommend running Codex in Windows Subsystem for Linux (WSL2)".bold(),
            ]),
            Line::from(vec!["  ".into(), "WSL allows Codex to run Agent mode in a sandboxed environment with better data protections in place.".into()]),
            Line::from(vec!["  ".into(), "Learn more: https://developers.openai.com/codex/windows".into()]),
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
            WindowsSetupSelection::Install,
            "Exit and install WSL2",
        ));
        lines.push(create_option(
            1,
            WindowsSetupSelection::Continue,
            "Continue anyway",
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
                self.highlighted = WindowsSetupSelection::Install;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlighted = WindowsSetupSelection::Continue;
            }
            KeyCode::Char('1') => self.handle_install(),
            KeyCode::Char('2') => self.handle_continue(),
            KeyCode::Enter => match self.highlighted {
                WindowsSetupSelection::Install => self.handle_install(),
                WindowsSetupSelection::Continue => self.handle_continue(),
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
