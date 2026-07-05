use std::io::IsTerminal;
use std::io::Result;
use std::io::stdin;
use std::io::stdout;
use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;

use super::CustomTerminal;
use super::InitializedTerminal;
use super::keyboard_modes;

const MAX_STARTUP_INPUT_CHARS: usize = 4096;

/// Discards terminal input if startup exits before the TUI takes ownership of it.
pub(crate) struct PreparedTerminal {
    active: bool,
    terminal_modes_active: bool,
}

impl Drop for PreparedTerminal {
    fn drop(&mut self) {
        if self.active {
            discard_terminal_input();
            if self.terminal_modes_active {
                let _ = super::restore_after_exit();
            }
        }
    }
}

#[derive(Default)]
pub(super) struct StartupInputBuffer {
    text: String,
    char_count: usize,
}

impl StartupInputBuffer {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) if modifiers.difference(KeyModifiers::SHIFT).is_empty() => match code {
                KeyCode::Char(ch) if !ch.is_control() => self.push_char(ch),
                KeyCode::Backspace => self.pop_char(),
                _ => {}
            },
            Event::Paste(text) => self.push_text(&text),
            _ => {}
        }
    }

    fn push_char(&mut self, ch: char) {
        if self.char_count < MAX_STARTUP_INPUT_CHARS {
            self.text.push(ch);
            self.char_count += 1;
        }
    }

    fn pop_char(&mut self) {
        if self.text.pop().is_some() {
            self.char_count -= 1;
        }
    }

    pub(super) fn push_text(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            self.push_char(ch);
        }
    }

    pub(super) fn handle_probe_input(&mut self, input: &[u8]) {
        for ch in String::from_utf8_lossy(input).chars() {
            match ch {
                '\u{8}' | '\u{7f}' => self.pop_char(),
                ch if !ch.is_control() => self.push_char(ch),
                _ => {}
            }
        }
    }

    pub(super) fn into_text(self) -> Option<String> {
        (!self.text.is_empty()).then_some(self.text)
    }
}

/// Flush the underlying stdin buffer to clear any input buffered at the terminal level.
#[cfg(unix)]
pub(super) fn flush_terminal_input_buffer() {
    // Safety: flushing the stdin queue is safe and does not move ownership.
    let result = unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!("failed to tcflush stdin: {err}");
    }
}

/// Flush the underlying stdin buffer to clear any input buffered at the terminal level.
#[cfg(windows)]
pub(super) fn flush_terminal_input_buffer() {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::FlushConsoleInputBuffer;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to get stdin handle for flush: error {err}");
        return;
    }

    let result = unsafe { FlushConsoleInputBuffer(handle) };
    if result == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to flush stdin buffer: error {err}");
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn flush_terminal_input_buffer() {}

pub(crate) fn discard_terminal_input() {
    flush_terminal_input_buffer();
}

pub(super) fn capture_startup_input(input: &mut StartupInputBuffer) -> Result<()> {
    while crossterm::event::poll(Duration::ZERO)? {
        input.handle_event(crossterm::event::read()?);
    }
    Ok(())
}

impl PreparedTerminal {
    /// Clear stale shell input before slower startup work begins.
    pub(crate) fn prepare() -> Result<Self> {
        if !stdin().is_terminal() {
            return Err(std::io::Error::other("stdin is not a terminal"));
        }
        if !stdout().is_terminal() {
            return Err(std::io::Error::other("stdout is not a terminal"));
        }
        discard_terminal_input();
        Ok(Self {
            active: true,
            terminal_modes_active: false,
        })
    }

    /// Initialize the TUI and move queued printable input into application-owned memory.
    pub(crate) fn activate(mut self) -> Result<InitializedTerminal> {
        self.terminal_modes_active = true;
        super::set_base_modes()?;
        let mut startup_input = StartupInputBuffer::default();
        capture_startup_input(&mut startup_input)?;
        super::set_panic_hook();

        #[cfg(unix)]
        let backend = CrosstermBackend::new(stdout());

        #[cfg(unix)]
        let startup_probe = {
            use crate::terminal_probe::StartupKeyboardEnhancementProbe;

            let started_at = std::time::Instant::now();
            let keyboard_probe = if keyboard_modes::keyboard_enhancement_disabled() {
                StartupKeyboardEnhancementProbe::Skip
            } else {
                StartupKeyboardEnhancementProbe::Query
            };
            match crate::terminal_probe::startup(
                crate::terminal_probe::DEFAULT_TIMEOUT,
                keyboard_probe,
            ) {
                Ok(probe) => {
                    tracing::info!(
                        duration_ms = %started_at.elapsed().as_millis(),
                        cursor_position = probe.cursor_position.is_some(),
                        default_colors = probe.default_colors.is_some(),
                        keyboard_enhancement_supported = ?probe.keyboard_enhancement_supported,
                        "terminal startup probes completed"
                    );
                    probe
                }
                Err(err) => {
                    tracing::warn!(
                        duration_ms = %started_at.elapsed().as_millis(),
                        "terminal startup probes failed: {err}"
                    );
                    crate::terminal_probe::StartupProbe {
                        cursor_position: None,
                        default_colors: None,
                        keyboard_enhancement_supported: None,
                        input: Vec::new(),
                    }
                }
            }
        };

        #[cfg(unix)]
        startup_input.handle_probe_input(&startup_probe.input);

        #[cfg(unix)]
        crate::terminal_palette::set_default_colors_from_startup_probe(
            startup_probe.default_colors,
        );

        #[cfg(unix)]
        let cursor_pos = match startup_probe.cursor_position {
            Some(pos) => pos,
            None => {
                tracing::warn!("initial cursor position probe timed out; defaulting to origin");
                Position { x: 0, y: 0 }
            }
        };

        #[cfg(unix)]
        let enhanced_keys_supported = startup_probe
            .keyboard_enhancement_supported
            .unwrap_or(/*default*/ false);

        #[cfg(not(unix))]
        let mut backend = CrosstermBackend::new(stdout());

        #[cfg(not(unix))]
        let cursor_pos = super::cursor_position_with_crossterm(&mut backend);

        #[cfg(not(unix))]
        let enhanced_keys_supported = !keyboard_modes::keyboard_enhancement_disabled()
            && super::detect_keyboard_enhancement_supported();

        #[cfg(windows)]
        super::probe_windows_default_colors();

        super::set_event_modes();
        capture_startup_input(&mut startup_input)?;

        let terminal = CustomTerminal::with_options_and_cursor_position(backend, cursor_pos)?;
        let stderr_guard = super::terminal_stderr::TerminalStderrGuard::install()?;
        let initialized = InitializedTerminal {
            terminal,
            enhanced_keys_supported,
            stderr_guard,
            startup_text: startup_input.into_text(),
        };
        self.active = false;
        Ok(initialized)
    }
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
