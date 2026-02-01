//! Key event handler for mapping keyboard input to commands.
//!
//! This module converts raw crossterm key events into high-level
//! [`TuiCommand`]s that can be processed by the application.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use super::TuiCommand;

/// Handle a key event and return the corresponding command.
///
/// This function maps keyboard input to application commands based on
/// the current focus state and modifiers.
///
/// # Arguments
///
/// * `key` - The key event to handle
/// * `has_overlay` - Whether an overlay (e.g., permission prompt) is active
///
/// # Returns
///
/// The command to execute, if any.
pub fn handle_key_event(key: KeyEvent, has_overlay: bool) -> Option<TuiCommand> {
    // Handle overlay-specific keys first
    if has_overlay {
        return handle_overlay_key(key);
    }

    // Handle global shortcuts (with modifiers)
    if let Some(cmd) = handle_global_key(key) {
        return Some(cmd);
    }

    // Handle input editing keys
    handle_input_key(key)
}

/// Handle keys when an overlay (permission prompt, model picker) is active.
fn handle_overlay_key(key: KeyEvent) -> Option<TuiCommand> {
    match key.code {
        // Approval shortcuts
        KeyCode::Char('y') | KeyCode::Char('Y') => Some(TuiCommand::Approve),
        KeyCode::Char('n') | KeyCode::Char('N') => Some(TuiCommand::Deny),
        KeyCode::Char('a') | KeyCode::Char('A')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            Some(TuiCommand::ApproveAll)
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(TuiCommand::CursorUp),
        KeyCode::Down | KeyCode::Char('j') => Some(TuiCommand::CursorDown),
        KeyCode::Enter => Some(TuiCommand::Approve),

        // Cancel
        KeyCode::Esc => Some(TuiCommand::Cancel),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(TuiCommand::Cancel)
        }

        _ => None,
    }
}

/// Handle global shortcuts that work regardless of focus.
fn handle_global_key(key: KeyEvent) -> Option<TuiCommand> {
    match (key.modifiers, key.code) {
        // Plan mode toggle (Tab)
        (KeyModifiers::NONE, KeyCode::Tab) => Some(TuiCommand::TogglePlanMode),

        // Thinking level cycle (Ctrl+T)
        (KeyModifiers::CONTROL, KeyCode::Char('t')) => Some(TuiCommand::CycleThinkingLevel),

        // Model cycle/picker (Ctrl+M)
        (KeyModifiers::CONTROL, KeyCode::Char('m')) => Some(TuiCommand::CycleModel),

        // Interrupt (Ctrl+C)
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(TuiCommand::Interrupt),

        // Clear screen (Ctrl+L)
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => Some(TuiCommand::ClearScreen),

        // External editor (Ctrl+E)
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => Some(TuiCommand::OpenExternalEditor),

        // Quit (Ctrl+Q)
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(TuiCommand::Quit),

        // Cancel (Escape)
        (_, KeyCode::Esc) => Some(TuiCommand::Cancel),

        // Page up/down with modifiers
        (_, KeyCode::PageUp) => Some(TuiCommand::PageUp),
        (_, KeyCode::PageDown) => Some(TuiCommand::PageDown),
        (KeyModifiers::CONTROL, KeyCode::Up) => Some(TuiCommand::PageUp),
        (KeyModifiers::CONTROL, KeyCode::Down) => Some(TuiCommand::PageDown),

        _ => None,
    }
}

/// Handle input editing keys.
fn handle_input_key(key: KeyEvent) -> Option<TuiCommand> {
    match (key.modifiers, key.code) {
        // Submit (Enter without modifiers, or Ctrl+Enter)
        (KeyModifiers::NONE, KeyCode::Enter) => Some(TuiCommand::SubmitInput),
        (KeyModifiers::CONTROL, KeyCode::Enter) => Some(TuiCommand::SubmitInput),

        // Newline (Shift+Enter, or Alt+Enter)
        (KeyModifiers::SHIFT, KeyCode::Enter) => Some(TuiCommand::InsertNewline),
        (KeyModifiers::ALT, KeyCode::Enter) => Some(TuiCommand::InsertNewline),

        // Character input
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            Some(TuiCommand::InsertChar(c))
        }

        // Backspace
        (KeyModifiers::NONE, KeyCode::Backspace) => Some(TuiCommand::DeleteBackward),
        (KeyModifiers::CONTROL, KeyCode::Backspace) => Some(TuiCommand::DeleteBackward), // TODO: Delete word

        // Delete
        (KeyModifiers::NONE, KeyCode::Delete) => Some(TuiCommand::DeleteForward),

        // Cursor movement
        (KeyModifiers::NONE, KeyCode::Left) => Some(TuiCommand::CursorLeft),
        (KeyModifiers::NONE, KeyCode::Right) => Some(TuiCommand::CursorRight),
        (KeyModifiers::NONE, KeyCode::Up) => Some(TuiCommand::CursorUp),
        (KeyModifiers::NONE, KeyCode::Down) => Some(TuiCommand::CursorDown),
        (KeyModifiers::NONE, KeyCode::Home) => Some(TuiCommand::CursorHome),
        (KeyModifiers::NONE, KeyCode::End) => Some(TuiCommand::CursorEnd),

        // Word movement (Ctrl+Arrow)
        (KeyModifiers::CONTROL, KeyCode::Left) => Some(TuiCommand::CursorHome), // TODO: Word left
        (KeyModifiers::CONTROL, KeyCode::Right) => Some(TuiCommand::CursorEnd), // TODO: Word right

        // Scroll (without modifiers, for chat area)
        (KeyModifiers::ALT, KeyCode::Up) => Some(TuiCommand::ScrollUp),
        (KeyModifiers::ALT, KeyCode::Down) => Some(TuiCommand::ScrollDown),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
    }

    #[test]
    fn test_tab_toggles_plan_mode() {
        let event = key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::TogglePlanMode)
        );
    }

    #[test]
    fn test_ctrl_t_cycles_thinking() {
        let event = key(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::CycleThinkingLevel)
        );
    }

    #[test]
    fn test_ctrl_m_cycles_model() {
        let event = key(KeyCode::Char('m'), KeyModifiers::CONTROL);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::CycleModel));
    }

    #[test]
    fn test_ctrl_c_interrupts() {
        let event = key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::Interrupt));
    }

    #[test]
    fn test_enter_submits() {
        let event = key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::SubmitInput)
        );
    }

    #[test]
    fn test_shift_enter_inserts_newline() {
        let event = key(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::InsertNewline)
        );
    }

    #[test]
    fn test_char_inserts() {
        let event = key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::InsertChar('a'))
        );
    }

    #[test]
    fn test_overlay_y_approves() {
        let event = key(KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(handle_key_event(event, true), Some(TuiCommand::Approve));
    }

    #[test]
    fn test_overlay_n_denies() {
        let event = key(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(handle_key_event(event, true), Some(TuiCommand::Deny));
    }

    #[test]
    fn test_escape_cancels() {
        let event = key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::Cancel));
    }
}
