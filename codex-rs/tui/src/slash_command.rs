use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
    New,
    Compact,
    Diff,
    Quit,
    ToggleMouseMode,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::New => "Start a new chat.",
            SlashCommand::Compact => {
                "Summarize and compact the current conversation to free up context."
            }
            SlashCommand::ToggleMouseMode => {
                "Toggle mouse mode (enable for scrolling, disable for text selection)"
            }
            SlashCommand::Quit => "Exit the application.",
            SlashCommand::Diff => {
                "Show git diff of the working directory (including untracked files)"
            }
        }
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter().map(|c| (c.command(), c)).collect()
}

#[cfg(test)]
mod tests {
    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::chat_composer::ChatComposer;
    use crossterm::event::KeyCode;
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::sync::mpsc;

    #[test]
    fn test_slash_commands() {
        let (tx, _rx) = mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender);

        let mut terminal = match Terminal::new(TestBackend::new(100, 10)) {
            Ok(t) => t,
            Err(e) => panic!("Failed to create terminal: {e}"),
        };

        // Initial empty state
        if let Err(e) = terminal.draw(|f| f.render_widget_ref(&composer, f.area())) {
            panic!("Failed to draw empty composer: {e}");
        }
        assert_snapshot!("empty_slash", terminal.backend());

        // Type slash to show commands
        let _ = composer.handle_key_event(crossterm::event::KeyEvent::new(
            KeyCode::Char('/'),
            crossterm::event::KeyModifiers::empty(),
        ));
        if let Err(e) = terminal.draw(|f| f.render_widget_ref(&composer, f.area())) {
            panic!("Failed to draw slash commands: {e}");
        }
        assert_snapshot!("slash_commands", terminal.backend());

        // Type 'c' to filter to compact
        let _ = composer.handle_key_event(crossterm::event::KeyEvent::new(
            KeyCode::Char('c'),
            crossterm::event::KeyModifiers::empty(),
        ));
        if let Err(e) = terminal.draw(|f| f.render_widget_ref(&composer, f.area())) {
            panic!("Failed to draw filtered commands: {e}");
        }
        assert_snapshot!("compact_filtered", terminal.backend());

        // Select compact command - we don't check the final state since it's handled by the app layer
        let _ = composer.handle_key_event(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::empty(),
        ));
    }
}
