use std::io::Result;
use std::io::Stdout;
use std::io::stdout;

use codex_core::config::Config;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;

use crate::custom_terminal::Terminal;

static KITTY_SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

pub fn kitty_keyboard_supported() -> Option<bool> {
    KITTY_SUPPORTED.get().copied()
}

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init(_config: &Config) -> Result<Tui> {
    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
    if KITTY_SUPPORTED.get().is_none() {
        if let Some(supported) = try_detect_kitty_keyboard_support()? {
            let _ = KITTY_SUPPORTED.set(supported);
        }
    }
    // Enable keyboard enhancement flags so modifiers for keys like Enter are disambiguated.
    // chat_composer.rs is using a keyboard event listener to enter for any modified keys
    // to create a new line that require this.
    execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    )?;
    set_panic_hook();

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
    execute!(stdout(), PopKeyboardEnhancementFlags)?;
    execute!(stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    Ok(())
}

/// Try to detect whether the terminal supports the kitty keyboard protocol by
/// sending the query sequence and scanning stdin for a matching reply.
/// Returns Ok(Some(true/false)) when detection could be performed, or Ok(None)
/// when detection is unsupported on this platform.
fn try_detect_kitty_keyboard_support() -> std::io::Result<Option<bool>> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::time::Duration;
        use std::time::Instant;
        // Send the query: CSI ? u
        let mut out = stdout();
        out.write_all(b"\x1b[?u")?;
        out.flush()?;

        // Read a short burst from stdin in nonblocking mode looking for
        // a reply of the form ESC [ ? <digits> u
        let fd = std::io::stdin();
        use std::os::fd::AsRawFd;
        let raw_fd = fd.as_raw_fd();

        // Set O_NONBLOCK
        let mut original_flags: Option<i32> = None;
        unsafe {
            let flags = libc::fcntl(raw_fd, libc::F_GETFL);
            if flags >= 0 {
                original_flags = Some(flags);
                let _ = libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        let start = Instant::now();
        let timeout = Duration::from_millis(60);
        let mut buf = [0u8; 1024];
        let mut collected: Vec<u8> = Vec::with_capacity(1024);

        while start.elapsed() < timeout {
            let n = unsafe { libc::read(raw_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n > 0 {
                let n = n as usize;
                collected.extend_from_slice(&buf[..n]);
                if parse_kitty_query_reply(&collected) {
                    // Restore flags best-effort
                    unsafe {
                        if let Some(flags) = original_flags {
                            let _ = libc::fcntl(raw_fd, libc::F_SETFL, flags);
                        }
                    }
                    return Ok(Some(true));
                }
            } else {
                // EAGAIN or no data
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        // Restore flags best-effort
        unsafe {
            if let Some(flags) = original_flags {
                let _ = libc::fcntl(raw_fd, libc::F_SETFL, flags);
            }
        }
        Ok(Some(false))
    }
    #[cfg(not(unix))]
    {
        Ok(None)
    }
}

/// Scan collected bytes for a kitty keyboard protocol reply: ESC [ ? <digits> u
#[cfg(unix)]
fn parse_kitty_query_reply(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i] == 0x1b && i + 3 < bytes.len() && bytes[i + 1] == b'[' && bytes[i + 2] == b'?' {
            let mut j = i + 3;
            let mut saw_digit = false;
            while j < bytes.len() {
                let b = bytes[j];
                if b.is_ascii_digit() {
                    saw_digit = true;
                    j += 1;
                    continue;
                }
                if b == b'u' {
                    return saw_digit; // found ESC [ ? <digits> u
                }
                break;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    false
}
