use std::io::Result;
use std::io::Stdout;
use std::io::stdout;

use codex_core::config::Config;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use ratatui::Terminal;
use ratatui::TerminalOptions;
use ratatui::Viewport;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init(_config: &Config) -> Result<Tui> {
    // For Termux compatibility, handle bracketed paste mode more gracefully
    let is_termux = std::env::var("TERMUX_VERSION").is_ok() || 
                   std::env::var("PREFIX").map_or(false, |p| p.contains("com.termux"));
    
    if !is_termux {
        // Some terminals may not support bracketed paste, so don't fail on error
        let _ = execute!(stdout(), EnableBracketedPaste);
    }

    enable_raw_mode()?;
    set_panic_hook();

    // Reserve a fixed number of lines for the interactive viewport (composer,
    // status, popups). History is injected above using `insert_before`. This
    // is an initial step of the refactor – later the height can become
    // dynamic. For now a conservative default keeps enough room for the
    // multi‑line composer while not occupying the whole screen.
    const BOTTOM_VIEWPORT_HEIGHT: u16 = 8;
    let backend = CrosstermBackend::new(stdout());
    let tui = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(BOTTOM_VIEWPORT_HEIGHT),
        },
    )?;
    Ok(tui)
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
    // For Termux compatibility, handle bracketed paste mode gracefully
    let is_termux = std::env::var("TERMUX_VERSION").is_ok() || 
                   std::env::var("PREFIX").map_or(false, |p| p.contains("com.termux"));
    
    if !is_termux {
        let _ = execute!(stdout(), DisableBracketedPaste);
    }
    disable_raw_mode()?;
    Ok(())
}
