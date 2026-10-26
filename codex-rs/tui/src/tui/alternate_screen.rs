//! Pair alternate-screen transitions with that screen's independent keyboard-mode stack.
//!
//! Push only on entry and pop before leaving or yielding input to an editor. The main screen
//! keeps its own TUI mode until the handoff returns to it. Transcript surfaces retain pointer
//! reporting across overlays and disable it before yielding. Promoting an overlay must not push
//! another keyboard frame. Refresh tmux's input policy on entry for mouse-capture requests.

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
use super::tmux;

// Panic and exit cleanup cannot borrow Tui. Track the actual screen independently of its owner.
pub(super) static ALTERNATE_SCREEN: AlternateScreen = AlternateScreen {
    active: AtomicBool::new(/*v*/ false),
    mouse_active: AtomicBool::new(/*v*/ false),
    mouse_capture_disabled: AtomicBool::new(/*v*/ false),
    input_configured: AtomicBool::new(/*v*/ false),
    keyboard_active: AtomicBool::new(/*v*/ false),
    parent_reports_key_releases: AtomicBool::new(/*v*/ false),
    link_pointer: super::link_pointer::LinkPointer::new(),
};

#[derive(Default)]
pub(super) struct AlternateScreen {
    link_pointer: super::link_pointer::LinkPointer,
    active: AtomicBool,
    mouse_active: AtomicBool,
    // Refresh alongside keyboard modes whenever this screen is entered or restored.
    mouse_capture_disabled: AtomicBool,
    // A cleanup/setup error must not make the next identical request look already applied.
    input_configured: AtomicBool,
    // An editor can take over the screen after Codex has popped its keyboard mode.
    keyboard_active: AtomicBool,
    // Restore the main screen's input classification when this screen's keyboard frame is popped.
    parent_reports_key_releases: AtomicBool,
}

/// Report pointer motion so the owned transcript can update its return-to-bottom hover state.
struct EnablePointerCapture;

impl Command for EnablePointerCapture {
    fn write_ansi(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        writer.write_str("\x1b[?1000h\x1b[?1002h\x1b[?1003h")
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
    pub(super) fn set_link_pointer(&self, writer: &mut impl Write, over_link: bool) -> Result<()> {
        if self.mouse_active.load(Ordering::Relaxed) {
            self.link_pointer.update(writer, over_link)
        } else {
            self.link_pointer
                .restore(writer, codex_terminal_detection::terminal_info().name)
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub(super) fn enter(&self, writer: &mut impl Write, capture_mouse: bool) -> Result<()> {
        queue!(writer, EnterAlternateScreen)?;
        // Stdout retains queued bytes on a flush error; cleanup must follow that pending entry.
        self.active.store(/*val*/ true, Ordering::Relaxed);
        writer.flush()?;
        self.parent_reports_key_releases
            .store(keyboard_modes::reports_key_releases(), Ordering::Relaxed);
        let mouse_capture = keyboard_modes::enable_keyboard_enhancement(writer);
        self.keyboard_active.store(
            !cfg!(windows) && !keyboard_modes::keyboard_enhancement_disabled(),
            Ordering::Relaxed,
        );
        self.mouse_capture_disabled.store(
            mouse_capture == tmux::MouseCapture::DisabledByTmux,
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
            execute!(writer, DisableAlternateScroll, EnablePointerCapture).and_then(|()| {
                // Some Windows terminals send legacy mouse reports as key records. Request SGR
                // reports so ConPTY can translate them into mouse records instead.
                writer.write_all(b"\x1b[?1006h")?;
                writer.flush()
            })
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
        let pointer_result = self
            .link_pointer
            .restore(writer, codex_terminal_detection::terminal_info().name);
        if self.mouse_active.load(Ordering::Relaxed) {
            let result = execute!(writer, DisableMouseCapture);
            #[cfg(windows)]
            let result = {
                // Attempt this even when restoring the console mode fails.
                let encoding_result = writer
                    .write_all(b"\x1b[?1006l")
                    .and_then(|()| writer.flush());
                result.and(encoding_result)
            };
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
        pointer_result
    }

    /// Release input modes without changing the screen, so an editor can take it over.
    pub(super) fn release_input(&self, writer: &mut impl Write) -> Result<()> {
        self.input_configured
            .store(/*val*/ false, Ordering::Relaxed);
        let mouse_result = self.disable_mouse(writer);
        // Crossterm never pushes a keyboard stack on native Windows: its input-record API
        // already reports enhanced keys, and its push/pop commands return Unsupported.
        let keyboard_result = if self.keyboard_active.load(Ordering::Relaxed) {
            // modifyOtherKeys is not stacked per screen; keep the main screen's fallback enabled
            // until terminal handoff restores its keyboard modes too.
            let result = execute!(writer, PopKeyboardEnhancementFlags);
            if result.is_ok() {
                self.keyboard_active.store(/*val*/ false, Ordering::Relaxed);
                keyboard_modes::set_reports_key_releases(
                    self.parent_reports_key_releases.load(Ordering::Relaxed),
                );
            }
            result
        } else {
            Ok(())
        };
        let scroll_result = execute!(writer, DisableAlternateScroll);
        mouse_result.and(keyboard_result).and(scroll_result)
    }

    pub(super) fn leave(&self, writer: &mut impl Write) -> Result<()> {
        let input_result = self.release_input(writer);
        // A failed earlier cleanup write must not prevent the actual screen transition.
        let screen_result = execute!(writer, LeaveAlternateScreen);
        if screen_result.is_ok() {
            self.active.store(/*val*/ false, Ordering::Relaxed);
        }
        input_result.and(screen_result)
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

impl super::MouseCapture {
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
            *self = Self::Inherit;
            let _ = screen.configure_input(writer, owned);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "alternate_screen_tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "alternate_screen_windows_tests.rs"]
mod windows_tests;
