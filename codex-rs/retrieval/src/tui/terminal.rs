//! Terminal setup and teardown for the retrieval TUI.
//!
//! Provides utilities for initializing and restoring the terminal state,
//! following the same patterns as codex-tui2.

use std::io;
use std::io::Stdout;
use std::panic;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Type alias for our terminal backend.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI mode.
///
/// This sets up:
/// - Raw mode (disable line buffering, echo)
/// - Alternate screen (preserve original terminal content)
/// - Bracketed paste mode (handle pasted text correctly)
/// - Panic hook to restore terminal on crash
pub fn init() -> io::Result<Tui> {
    // Install panic hook to restore terminal on crash
    install_panic_hook();

    // Enable raw mode
    enable_raw_mode()?;

    // Enter alternate screen and enable bracketed paste
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;

    // Create terminal
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

/// Restore the terminal to its original state.
///
/// This should be called when exiting the TUI, either normally or on error.
pub fn restore() -> io::Result<()> {
    // Leave alternate screen and disable bracketed paste
    execute!(io::stdout(), LeaveAlternateScreen, DisableBracketedPaste)?;

    // Disable raw mode
    disable_raw_mode()?;

    Ok(())
}

/// Install a panic hook that restores the terminal.
///
/// This ensures the terminal is restored even if the application panics,
/// preventing the user from being stuck in raw mode.
fn install_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restore
        let _ = restore();

        // Call original panic handler
        original_hook(panic_info);
    }));
}

/// RAII guard that restores the terminal when dropped.
///
/// Use this to ensure terminal restoration even with early returns or errors.
///
/// # Example
///
/// ```ignore
/// let _guard = TerminalGuard::new()?;
/// // TUI operations...
/// // Terminal is automatically restored when _guard goes out of scope
/// ```
pub struct TerminalGuard {
    terminal: Option<Tui>,
}

impl TerminalGuard {
    /// Create a new terminal guard, initializing the terminal.
    pub fn new() -> io::Result<Self> {
        let terminal = init()?;
        Ok(Self {
            terminal: Some(terminal),
        })
    }

    /// Get a mutable reference to the terminal.
    pub fn terminal(&mut self) -> Option<&mut Tui> {
        self.terminal.as_mut()
    }

    /// Take ownership of the terminal, disabling auto-restore.
    pub fn take(mut self) -> Option<Tui> {
        self.terminal.take()
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.terminal.is_some() {
            let _ = restore();
        }
    }
}

#[cfg(test)]
mod tests {
    // Terminal tests are difficult to run in CI, so we just verify compilation
    #[test]
    fn test_types_exist() {
        // Just ensure the types compile
        fn _check_types() {
            let _: fn() -> std::io::Result<super::Tui> = super::init;
            let _: fn() -> std::io::Result<()> = super::restore;
        }
    }
}
