//! Helper functions for checking if exit keys are pressed
use crossterm::event::{self, KeyCode};

pub fn is_exit_key_pressed() -> std::io::Result<bool> {
    Ok(event::read()?
        .as_key_press_event()
        .is_some_and(|key| matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))))
}
