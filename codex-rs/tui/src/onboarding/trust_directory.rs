use std::path::PathBuf;

use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::key_hint::KeyBindingListExt;
use crate::onboarding::keys;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;

use super::onboarding_screen::StepState;
pub(crate) struct TrustDirectoryWidget {
    pub cwd: PathBuf,
    pub trust_target: PathBuf,
    pub show_windows_create_sandbox_hint: bool,
    pub should_quit: bool,
    pub selection: Option<TrustDirectorySelection>,
    pub highlighted: TrustDirectorySelection,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustDirectorySelection {
    Trust,
    Quit,
}

impl WidgetRef for &TrustDirectoryWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut column = ColumnRenderable::new();

        column.push(Line::from(vec![
            "> ".into(),
            "You are in ".bold(),
            self.cwd.to_string_lossy().to_string().into(),
        ]));
        column.push("");

        if self.cwd != self.trust_target {
            #[allow(clippy::disallowed_methods)]
            let git_root_warning = Paragraph::new(format!(
                "Note: You’re in a subdirectory of a Git project. Trusting will apply to the repository root: {}",
                self.trust_target.display()
            ))
            .yellow();
            column.push(
                git_root_warning
                    .wrap(Wrap { trim: true })
                    .inset(Insets::tlbr(
                        /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
                    )),
            );
            column.push("");
        }

        column.push(
            Paragraph::new(
                "Do you trust the contents of this directory? Working with untrusted \
                 contents comes with higher risk of prompt injection. Trusting the \
                 directory allows project-local config, hooks, and exec policies to load."
                    .to_string(),
            )
            .wrap(Wrap { trim: true })
            .inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        column.push("");

        let options: Vec<(&str, TrustDirectorySelection)> = vec![
            ("Yes, continue", TrustDirectorySelection::Trust),
            ("No, quit", TrustDirectorySelection::Quit),
        ];

        for (idx, (text, selection)) in options.iter().enumerate() {
            column.push(selection_option_row(
                idx,
                text.to_string(),
                self.highlighted == *selection,
            ));
        }

        column.push("");

        if let Some(error) = &self.error {
            column.push(
                Paragraph::new(error.to_string())
                    .red()
                    .wrap(Wrap { trim: true })
                    .inset(Insets::tlbr(
                        /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
                    )),
            );
            column.push("");
        }

        column.push(
            Line::from(vec![
                "Press ".dim(),
                keys::CONFIRM[0].into(),
                if self.show_windows_create_sandbox_hint {
                    " to continue and create a sandbox...".dim()
                } else {
                    " to continue".dim()
                },
            ])
            .inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );

        column.render(area, buf);
    }
}

impl KeyboardHandler for TrustDirectoryWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind != KeyEventKind::Press {
            return;
        }

        if keys::MOVE_UP.is_pressed(key_event) {
            self.highlighted = TrustDirectorySelection::Trust;
        } else if keys::MOVE_DOWN.is_pressed(key_event) {
            self.highlighted = TrustDirectorySelection::Quit;
        } else if keys::SELECT_FIRST.is_pressed(key_event) {
            // A terminal response fragment can start with `1`; trust always requires an explicit
            // Enter confirmation after the directory prompt is visible.
            self.highlighted = TrustDirectorySelection::Trust;
        } else if keys::SELECT_SECOND.is_pressed(key_event)
            || keys::QUIT.is_pressed(key_event)
            || keys::CANCEL.is_pressed(key_event)
        {
            self.handle_quit();
        } else if keys::CONFIRM.is_pressed(key_event) {
            match self.highlighted {
                TrustDirectorySelection::Trust => self.handle_trust(),
                TrustDirectorySelection::Quit => self.handle_quit(),
            }
        }
    }
}

impl StepStateProvider for TrustDirectoryWidget {
    fn get_step_state(&self) -> StepState {
        if self.selection.is_some() || self.should_quit {
            StepState::Complete
        } else {
            StepState::InProgress
        }
    }
}

impl TrustDirectoryWidget {
    fn handle_trust(&mut self) {
        self.highlighted = TrustDirectorySelection::Trust;
        self.error = None;
        self.selection = Some(TrustDirectorySelection::Trust);
    }

    fn handle_quit(&mut self) {
        self.highlighted = TrustDirectorySelection::Quit;
        self.should_quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}

#[cfg(test)]
mod tests {
    use crate::test_backend::VT100Backend;

    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn widget(error: Option<String>) -> TrustDirectoryWidget {
        TrustDirectoryWidget {
            cwd: PathBuf::from("/workspace/project"),
            trust_target: PathBuf::from("/workspace/project"),
            show_windows_create_sandbox_hint: false,
            should_quit: false,
            selection: None,
            highlighted: TrustDirectorySelection::Trust,
            error,
        }
    }

    #[test]
    fn release_event_does_not_change_selection() {
        let mut widget = TrustDirectoryWidget {
            cwd: PathBuf::from("."),
            trust_target: PathBuf::from("."),
            show_windows_create_sandbox_hint: false,
            should_quit: false,
            selection: None,
            highlighted: TrustDirectorySelection::Quit,
            error: None,
        };

        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        };
        widget.handle_key_event(release);
        assert_eq!(widget.selection, None);

        let repeat =
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat);
        widget.handle_key_event(repeat);
        assert_eq!(widget.selection, None);

        let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        widget.handle_key_event(press);
        assert!(widget.should_quit);
    }

    #[test]
    fn fragmented_terminal_response_cannot_grant_directory_trust() {
        let mut widget = widget(/*error*/ None);
        widget.highlighted = TrustDirectorySelection::Quit;

        // The prefix may have been consumed by the protected-screen input drain, leaving the
        // numeric OSC slot as the first key delivered after the trust prompt becomes active.
        for character in "10;rgb:ffff/ffff/ffff".chars() {
            widget.handle_key_event(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }

        assert_eq!(widget.selection, None);
        assert_eq!(widget.highlighted, TrustDirectorySelection::Trust);

        widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(widget.selection, Some(TrustDirectorySelection::Trust));
    }

    #[test]
    fn renders_snapshot_for_git_repo() {
        let widget = widget(/*error*/ None);

        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 70, /*height*/ 14)).expect("terminal");
        terminal
            .draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))
            .expect("draw");

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_snapshot_for_remote_git_subdirectory() {
        let widget = TrustDirectoryWidget {
            cwd: PathBuf::from("/srv/remote/project/nested"),
            trust_target: PathBuf::from("/srv/remote/project"),
            ..widget(/*error*/ None)
        };

        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 70, /*height*/ 18)).expect("terminal");
        terminal
            .draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))
            .expect("draw");

        insta::assert_snapshot!(
            terminal
                .backend()
                .to_string()
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn renders_snapshot_for_trust_error() {
        let widget = widget(Some(
            "Failed to set trust for /workspace/project: config/batchWrite failed in TUI: Invalid configuration: features.fast_mode=true is not supported; allowed set [fast_mode=false]"
                .to_string(),
        ));

        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 70, /*height*/ 18)).expect("terminal");
        terminal
            .draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))
            .expect("draw");

        insta::assert_snapshot!(terminal.backend());
    }
}
