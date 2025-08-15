use std::io::Result;
use std::io::Stdout;
use std::io::stdout;

use codex_core::config::Config;
use crossterm::cursor::MoveTo;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;

use crate::custom_terminal::Terminal;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init(_config: &Config) -> Result<Tui> {
    set_modes()?;
    set_panic_hook();

    // Clear screen and move cursor to top-left before drawing UI
    execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;

    let backend = CrosstermBackend::new(stdout());
    let tui = Terminal::with_options(backend)?;
    Ok(tui)
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_modes(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

pub fn set_modes() -> Result<()> {
    execute!(stdout(), EnableBracketedPaste)?;
    enable_raw_mode()?;
    let _ = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    );
    Ok(())
}

/// Restore the terminal to its original state
pub fn restore_modes() -> Result<()> {
    // Pop may fail on platforms that didn't support the push; ignore errors.
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    Ok(())
}
