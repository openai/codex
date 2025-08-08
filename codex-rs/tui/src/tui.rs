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
    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
    // Enable keyboard enhancement flags so modifiers for keys like Enter are disambiguated.
    // chat_composer.rs is using a keyboard event listener to enter for any modified keys
    // to create a new line that require this.
    // Try to enable keyboard enhancement, but ignore errors on Windows legacy console
    // This is a known limitation with Windows terminal compatibility - progressive keyboard
    // enhancement is not supported on legacy Windows API, but the app works fine without it
    #[cfg(not(target_os = "windows"))]
    {
        let _ = execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        );
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, try to enable keyboard enhancement but silently ignore failures
        // This allows the app to work on legacy Windows terminals that don't support
        // progressive keyboard enhancement
        let _ = execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        );
    }
    
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
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    // Try to pop keyboard enhancement flags, but ignore errors on Windows legacy console
    // This matches the conditional logic in init() for Windows compatibility
    #[cfg(not(target_os = "windows"))]
    {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, try to pop keyboard enhancement but silently ignore failures
        // for compatibility with legacy terminals
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    
    execute!(stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    Ok(())
}
