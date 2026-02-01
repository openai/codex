//! Terminal setup and management.
//!
//! This module provides:
//! - Terminal initialization and restoration
//! - The main [`Tui`] struct for running the application
//! - Panic handler for terminal cleanup

use std::io::Stdout;
use std::io::{self};
use std::panic;
use std::sync::Arc;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableFocusChange;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::broadcast;

use crate::event::EventBroker;
use crate::event::TuiEventStream;
use crate::state::AppState;

/// Type alias for the terminal backend.
pub type TerminalBackend = CrosstermBackend<Stdout>;

/// Type alias for the ratatui terminal.
pub type RatatuiTerminal = Terminal<TerminalBackend>;

/// Set up the terminal for TUI mode.
///
/// This function:
/// - Enables raw mode (keypresses are not echoed, no line buffering)
/// - Enters the alternate screen (preserves user's scrollback)
/// - Enables bracketed paste mode (detect paste vs typing)
/// - Enables focus change events (detect when terminal gains/loses focus)
///
/// # Errors
///
/// Returns an error if terminal setup fails.
pub fn setup_terminal() -> io::Result<RatatuiTerminal> {
    // Check that stdin/stdout are terminals
    if !io::stdin().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "stdin is not a terminal",
        ));
    }
    if !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "stdout is not a terminal",
        ));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableFocusChange,
    )?;

    // Set up panic hook to restore terminal on panic
    set_panic_hook();

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

/// Restore the terminal to its original state.
///
/// This function:
/// - Disables bracketed paste and focus change events
/// - Leaves the alternate screen
/// - Disables raw mode
///
/// # Errors
///
/// Returns an error if terminal restoration fails.
pub fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        DisableFocusChange,
    )?;
    Ok(())
}

/// Install a panic hook that restores the terminal.
fn set_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Attempt to restore terminal, ignore errors
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
}

/// Check if stdout is a terminal.
trait IsTerminal {
    fn is_terminal(&self) -> bool;
}

impl IsTerminal for io::Stdin {
    fn is_terminal(&self) -> bool {
        std::io::IsTerminal::is_terminal(self)
    }
}

impl IsTerminal for io::Stdout {
    fn is_terminal(&self) -> bool {
        std::io::IsTerminal::is_terminal(self)
    }
}

/// The main TUI application.
///
/// This struct manages the terminal and provides the main event loop
/// for the TUI application.
pub struct Tui {
    /// The ratatui terminal.
    pub(crate) terminal: RatatuiTerminal,

    /// Event broker for stdin control.
    event_broker: Arc<EventBroker>,

    /// Sender for draw requests.
    draw_tx: broadcast::Sender<()>,

    /// Application state.
    state: AppState,

    /// Whether the TUI should use the alternate screen.
    use_alt_screen: bool,
}

impl Tui {
    /// Create a new TUI application.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub fn new() -> io::Result<Self> {
        let terminal = setup_terminal()?;
        let (draw_tx, _) = broadcast::channel(16);

        Ok(Self {
            terminal,
            event_broker: Arc::new(EventBroker::new()),
            draw_tx,
            state: AppState::new(),
            use_alt_screen: true,
        })
    }

    /// Create a TUI with an existing terminal (for testing).
    pub fn with_terminal(terminal: RatatuiTerminal) -> Self {
        let (draw_tx, _) = broadcast::channel(16);

        Self {
            terminal,
            event_broker: Arc::new(EventBroker::new()),
            draw_tx,
            state: AppState::new(),
            use_alt_screen: true,
        }
    }

    /// Set whether to use the alternate screen.
    pub fn set_use_alt_screen(&mut self, use_alt_screen: bool) {
        self.use_alt_screen = use_alt_screen;
    }

    /// Get a reference to the application state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get a mutable reference to the application state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Request a frame redraw.
    pub fn request_redraw(&self) {
        let _ = self.draw_tx.send(());
    }

    /// Create an event stream for this TUI.
    pub fn event_stream(&self) -> TuiEventStream {
        use std::sync::atomic::AtomicBool;
        TuiEventStream::new(
            self.event_broker.clone(),
            self.draw_tx.subscribe(),
            Arc::new(AtomicBool::new(true)),
        )
    }

    /// Pause stdin reading (for external editors).
    pub fn pause_events(&self) {
        self.event_broker.pause();
    }

    /// Resume stdin reading.
    pub fn resume_events(&self) {
        self.event_broker.resume();
    }

    /// Draw a frame.
    ///
    /// # Errors
    ///
    /// Returns an error if drawing fails.
    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    /// Clear the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if clearing fails.
    pub fn clear(&mut self) -> io::Result<()> {
        self.terminal.clear()
    }

    /// Get the terminal size.
    pub fn size(&self) -> io::Result<ratatui::layout::Rect> {
        let size = self.terminal.size()?;
        Ok(ratatui::layout::Rect::new(0, 0, size.width, size.height))
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        // Attempt to restore terminal, ignore errors
        let _ = restore_terminal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most terminal tests require an actual terminal and can't run in CI.
    // These tests focus on non-terminal-dependent functionality.

    #[test]
    fn test_app_state_default() {
        let state = AppState::new();
        assert!(!state.should_exit());
    }
}
