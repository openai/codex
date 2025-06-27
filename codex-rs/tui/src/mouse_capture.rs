use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use ratatui::crossterm::execute;
use std::io::Result;
use std::io::stdout;

pub(crate) struct MouseCapture {
    mouse_capture_is_active: bool,
}

impl MouseCapture {
    pub(crate) fn new_with_capture(mouse_capture_is_active: bool) -> Result<Self> {
        if mouse_capture_is_active {
            enable_capture()?;
        }

        Ok(Self {
            mouse_capture_is_active,
        })
    }
}

impl MouseCapture {
    /// Idempotent method to set the mouse capture state.
    pub fn set_active(&mut self, is_active: bool) -> Result<()> {
        match (self.mouse_capture_is_active, is_active) {
            (true, true) => {}
            (false, false) => {}
            (true, false) => {
                disable_capture()?;
                self.mouse_capture_is_active = false;
            }
            (false, true) => {
                enable_capture()?;
                self.mouse_capture_is_active = true;
            }
        }
        Ok(())
    }

    pub(crate) fn toggle(&mut self) -> Result<()> {
        self.set_active(!self.mouse_capture_is_active)
    }

    pub(crate) fn disable(&mut self) -> Result<()> {
        if self.mouse_capture_is_active {
            disable_capture()?;
            self.mouse_capture_is_active = false;
        }
        Ok(())
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        if self.disable().is_err() {
            // The user is likely shutting down, so ignore any errors so the
            // shutdown process can complete.
        }
    }
}

fn enable_capture() -> Result<()> {
    execute!(stdout(), EnableMouseCapture)
}

fn disable_capture() -> Result<()> {
    execute!(stdout(), DisableMouseCapture)
}
