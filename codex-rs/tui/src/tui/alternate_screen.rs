//! Pair alternate-screen transitions with that screen's independent keyboard-mode stack.
//!
//! Push only on entry and pop before leaving; the main screen keeps its own TUI mode until
//! terminal handoff. Transcript surfaces retain pointer reporting across overlays and disable it
//! before yielding to the shell. Promoting an overlay must not push another keyboard frame.
//! Refresh tmux's input policy on entry and apply it to every mouse-capture request.

use std::io::Result;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crossterm::Command;
use crossterm::event::DisableMouseCapture;
#[cfg(windows)]
use crossterm::event::EnableMouseCapture;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::execute;
use crossterm::queue;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;

use super::DisableAlternateScroll;
use super::EnableAlternateScroll;
use super::KeyboardRestore;
use super::keyboard_modes;
use super::tmux::MouseCapture;

// Panic and exit cleanup cannot borrow Tui. Track the actual screen independently of its owner.
pub(super) static ALTERNATE_SCREEN: AlternateScreen = AlternateScreen {
    active: AtomicBool::new(/*v*/ false),
    mouse_active: AtomicBool::new(/*v*/ false),
    mouse_capture_disabled: AtomicBool::new(/*v*/ false),
    input_configured: AtomicBool::new(/*v*/ false),
};

#[derive(Default)]
pub(super) struct AlternateScreen {
    active: AtomicBool,
    mouse_active: AtomicBool,
    // Refresh alongside keyboard modes whenever this screen is entered or restored.
    mouse_capture_disabled: AtomicBool,
    // A cleanup/setup error must not make the next identical request look already applied.
    input_configured: AtomicBool,
}

/// Report pointer motion so the owned transcript can update its return-to-bottom hover state.
struct EnablePointerCapture;

impl Command for EnablePointerCapture {
    fn write_ansi(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        writer.write_str("\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1003h")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        EnableMouseCapture.execute_winapi()
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

impl AlternateScreen {
    pub(super) fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub(super) fn enter(&self, writer: &mut impl Write, capture_mouse: bool) -> Result<()> {
        queue!(writer, EnterAlternateScreen)?;
        // Stdout retains queued bytes on a flush error; cleanup must follow that pending entry.
        self.active.store(/*val*/ true, Ordering::Relaxed);
        writer.flush()?;
        let mouse_capture = keyboard_modes::enable_keyboard_enhancement(writer);
        self.mouse_capture_disabled.store(
            mouse_capture == MouseCapture::DisabledByTmux,
            Ordering::Relaxed,
        );
        self.configure_input(writer, capture_mouse)
    }

    pub(super) fn configure_input(
        &self,
        writer: &mut impl Write,
        capture_mouse: bool,
    ) -> Result<()> {
        self.input_configured
            .store(/*val*/ false, Ordering::Relaxed);
        let result = if capture_mouse && !self.mouse_capture_disabled.load(Ordering::Relaxed) {
            // A partial write can already enable reporting; cleanup must still attempt to stop it.
            self.mouse_active.store(/*val*/ true, Ordering::Relaxed);
            execute!(writer, DisableAlternateScroll, EnablePointerCapture)
        } else {
            let mouse_result = self.disable_mouse(writer);
            let scroll_result = execute!(writer, EnableAlternateScroll);
            mouse_result.and(scroll_result)
        };
        if result.is_ok() {
            self.input_configured.store(/*val*/ true, Ordering::Relaxed);
        }
        result
    }

    fn disable_mouse(&self, writer: &mut impl Write) -> Result<()> {
        if self.mouse_active.load(Ordering::Relaxed) {
            let result = execute!(writer, DisableMouseCapture);
            if result.is_err() {
                // A partial combined write must not skip the remaining mode resets. Keep the
                // cleanup flag armed because delivery of these best-effort writes is uncertain.
                #[cfg(not(windows))]
                for sequence in [
                    b"\x1b[?1003l",
                    b"\x1b[?1002l",
                    b"\x1b[?1000l",
                    b"\x1b[?1006l",
                ] {
                    let _ = writer.write_all(sequence);
                }
                let _ = writer.flush();
                return result;
            }

            self.mouse_active.store(/*val*/ false, Ordering::Relaxed);
        }
        Ok(())
    }

    pub(super) fn leave(&self, writer: &mut impl Write) -> Result<()> {
        self.input_configured
            .store(/*val*/ false, Ordering::Relaxed);
        let mouse_result = self.disable_mouse(writer);
        // Crossterm never pushes a keyboard stack on native Windows: its input-record API
        // already reports enhanced keys, and its push/pop commands return Unsupported.
        let keyboard_result = if cfg!(windows) || keyboard_modes::keyboard_enhancement_disabled() {
            Ok(())
        } else {
            // modifyOtherKeys is not stacked per screen; keep the main screen's fallback enabled
            // until terminal handoff restores its keyboard modes too.
            execute!(writer, PopKeyboardEnhancementFlags)
        };
        let scroll_result = execute!(writer, DisableAlternateScroll);
        // A failed earlier cleanup write must not prevent the actual screen transition.
        let screen_result = execute!(writer, LeaveAlternateScreen);
        if screen_result.is_ok() {
            self.active.store(/*val*/ false, Ordering::Relaxed);
        }
        mouse_result
            .and(keyboard_result)
            .and(scroll_result)
            .and(screen_result)
    }

    pub(super) fn restore(
        &self,
        writer: &mut impl Write,
        keyboard_restore: KeyboardRestore,
    ) -> Result<()> {
        let screen_result = if self.active.load(Ordering::Relaxed) {
            self.leave(writer)
        } else {
            // A failed mouse cleanup can outlive a successful return to the main screen.
            self.disable_mouse(writer)
        };
        if self.active.load(Ordering::Relaxed) {
            // Leave failed: another pop here would still target the alternate stack.
            return screen_result;
        }
        // The alternate stack is restored before the main stack, even for legacy overlays.
        match keyboard_restore {
            KeyboardRestore::PopStack => keyboard_modes::restore_keyboard_enhancement_stack(writer),
            KeyboardRestore::ResetAfterExit => {
                keyboard_modes::reset_keyboard_reporting_after_exit(writer);
            }
        }
        screen_result
    }
}

impl super::OverlayInput {
    /// Roll back a partial overlay setup immediately; callers still receive the original error.
    /// The fallback keeps capture when the session owns the screen, otherwise restores the picker.
    pub(super) fn apply(
        &mut self,
        screen: &AlternateScreen,
        writer: &mut impl Write,
        next: Self,
        owned: bool,
    ) -> Result<()> {
        if *self == next && screen.input_configured.load(Ordering::Relaxed) {
            return Ok(());
        }
        *self = next;
        if let Err(error) = screen.configure_input(writer, next.captures_mouse(owned)) {
            *self = Self::Default;
            let _ = screen.configure_input(writer, owned);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "alternate_screen_tests.rs"]
mod tests;
