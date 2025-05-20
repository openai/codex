use std::io::Result;
use std::io::Stdout;
use std::io::stdout;

use codex_core::config::Config;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::EnterAlternateScreen;
use ratatui::crossterm::terminal::LeaveAlternateScreen;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;

use crate::mouse_capture::MouseCapture;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init(config: &Config) -> Result<(Tui, MouseCapture)> {
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), EnableBracketedPaste)?;
    let mouse_capture = MouseCapture::new_with_capture(!config.tui.disable_mouse_capture)?;

    enable_raw_mode()?;
    set_panic_hook();
    let tui = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok((tui, mouse_capture))
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    // We are shutting down, and we cannot reference the `MouseCapture`, so we
    // categorically disable mouse capture just to be safe.
    if execute!(stdout(), DisableMouseCapture).is_err() {
        // It is possible that `DisableMouseCapture` is written more than once
        // on shutdown, so ignore the error in this case.
    }
    execute!(stdout(), DisableBracketedPaste)?;
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
