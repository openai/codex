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
/// * `has_file_suggestions` - Whether file suggestions are being displayed
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

/// Handle a key event with file and skill suggestion state.
///
/// When suggestions are active, some keys are redirected to
/// suggestion navigation. Skill suggestions take priority over file suggestions.
pub fn handle_key_event_with_suggestions(
    key: KeyEvent,
    has_overlay: bool,
    has_file_suggestions: bool,
) -> Option<TuiCommand> {
    handle_key_event_full(key, has_overlay, has_file_suggestions, false, false)
}

/// Handle a key event with full context including streaming state.
///
/// This is the most complete handler that supports:
/// - Overlay handling
/// - File and skill suggestion navigation
/// - Queue/steering behavior based on streaming state
pub fn handle_key_event_full(
    key: KeyEvent,
    has_overlay: bool,
    has_file_suggestions: bool,
    has_skill_suggestions: bool,
    is_streaming: bool,
) -> Option<TuiCommand> {
    // Handle overlay-specific keys first
    if has_overlay {
        return handle_overlay_key(key);
    }

    // Handle skill suggestion navigation (higher priority)
    if has_skill_suggestions {
        if let Some(cmd) = handle_skill_suggestion_key(key) {
            return Some(cmd);
        }
    }

    // Handle file suggestion navigation
    if has_file_suggestions {
        if let Some(cmd) = handle_suggestion_key(key) {
            return Some(cmd);
        }
    }

    // Handle global shortcuts (with modifiers)
    if let Some(cmd) = handle_global_key(key) {
        return Some(cmd);
    }

    // Handle input editing keys with streaming context
    handle_input_key_with_streaming(key, is_streaming)
}

/// Handle keys for file suggestion navigation.
fn handle_suggestion_key(key: KeyEvent) -> Option<TuiCommand> {
    match (key.modifiers, key.code) {
        // Navigate suggestions
        (KeyModifiers::NONE, KeyCode::Up) => Some(TuiCommand::SelectPrevSuggestion),
        (KeyModifiers::NONE, KeyCode::Down) => Some(TuiCommand::SelectNextSuggestion),

        // Accept suggestion
        (KeyModifiers::NONE, KeyCode::Tab) => Some(TuiCommand::AcceptSuggestion),
        (KeyModifiers::NONE, KeyCode::Enter) => Some(TuiCommand::AcceptSuggestion),

        // Dismiss suggestions
        (KeyModifiers::NONE, KeyCode::Esc) => Some(TuiCommand::DismissSuggestions),

        // Other keys pass through to normal handling
        _ => None,
    }
}

/// Handle keys for skill suggestion navigation.
fn handle_skill_suggestion_key(key: KeyEvent) -> Option<TuiCommand> {
    match (key.modifiers, key.code) {
        // Navigate suggestions
        (KeyModifiers::NONE, KeyCode::Up) => Some(TuiCommand::SelectPrevSkillSuggestion),
        (KeyModifiers::NONE, KeyCode::Down) => Some(TuiCommand::SelectNextSkillSuggestion),

        // Accept suggestion
        (KeyModifiers::NONE, KeyCode::Tab) => Some(TuiCommand::AcceptSkillSuggestion),
        (KeyModifiers::NONE, KeyCode::Enter) => Some(TuiCommand::AcceptSkillSuggestion),

        // Dismiss suggestions
        (KeyModifiers::NONE, KeyCode::Esc) => Some(TuiCommand::DismissSkillSuggestions),

        // Other keys pass through to normal handling
        _ => None,
    }
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

        // Character input for filter-based overlays
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Some(TuiCommand::InsertChar(c))
        }

        // Backspace for filter
        KeyCode::Backspace => Some(TuiCommand::DeleteBackward),

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

        // Command palette (Ctrl+P)
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Some(TuiCommand::ShowCommandPalette),

        // Session browser (Ctrl+S)
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Some(TuiCommand::ShowSessionBrowser),

        // Toggle thinking display (Ctrl+Shift+T)
        (m, KeyCode::Char('T'))
            if m.contains(KeyModifiers::CONTROL) && m.contains(KeyModifiers::SHIFT) =>
        {
            Some(TuiCommand::ToggleThinking)
        }

        // Show help (? or F1)
        (KeyModifiers::NONE, KeyCode::F(1)) => Some(TuiCommand::ShowHelp),
        (KeyModifiers::SHIFT, KeyCode::Char('?')) => Some(TuiCommand::ShowHelp),

        // Quit (Ctrl+Q)
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(TuiCommand::Quit),

        // Smart paste from clipboard: image first, text fallback (Ctrl+V)
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => Some(TuiCommand::PasteFromClipboard),

        // Alt+V: Windows fallback where Ctrl+V may be intercepted by terminal
        (KeyModifiers::ALT, KeyCode::Char('v')) => Some(TuiCommand::PasteFromClipboard),

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
    // Delegate to streaming-aware handler with streaming=false
    handle_input_key_with_streaming(key, false)
}

/// Handle input editing keys with streaming context.
///
/// When `is_streaming` is true:
/// - Enter / Ctrl+Enter queues the input for later (QueueInput)
///
/// When `is_streaming` is false:
/// - Enter / Ctrl+Enter submits immediately (SubmitInput)
///
/// Both modes:
/// - Shift+Enter inserts a newline (for multi-line input)
/// - Alt+Enter inserts a newline (for multi-line input)
fn handle_input_key_with_streaming(key: KeyEvent, is_streaming: bool) -> Option<TuiCommand> {
    match (key.modifiers, key.code) {
        // Enter / Ctrl+Enter: Submit or Queue depending on streaming state
        (KeyModifiers::NONE | KeyModifiers::CONTROL, KeyCode::Enter) => {
            if is_streaming {
                Some(TuiCommand::QueueInput)
            } else {
                Some(TuiCommand::SubmitInput)
            }
        }

        // Shift+Enter: Insert newline (aligned with Claude Code behavior)
        (KeyModifiers::SHIFT, KeyCode::Enter) => Some(TuiCommand::InsertNewline),

        // Alt+Enter: Insert newline (for multi-line input)
        (KeyModifiers::ALT, KeyCode::Enter) => Some(TuiCommand::InsertNewline),

        // Character input
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            Some(TuiCommand::InsertChar(c))
        }

        // Backspace
        (KeyModifiers::NONE, KeyCode::Backspace) => Some(TuiCommand::DeleteBackward),
        (KeyModifiers::CONTROL, KeyCode::Backspace) => Some(TuiCommand::DeleteWordBackward),

        // Delete
        (KeyModifiers::NONE, KeyCode::Delete) => Some(TuiCommand::DeleteForward),
        (KeyModifiers::CONTROL, KeyCode::Delete) => Some(TuiCommand::DeleteWordForward),

        // Cursor movement
        (KeyModifiers::NONE, KeyCode::Left) => Some(TuiCommand::CursorLeft),
        (KeyModifiers::NONE, KeyCode::Right) => Some(TuiCommand::CursorRight),
        (KeyModifiers::NONE, KeyCode::Up) => Some(TuiCommand::CursorUp),
        (KeyModifiers::NONE, KeyCode::Down) => Some(TuiCommand::CursorDown),
        (KeyModifiers::NONE, KeyCode::Home) => Some(TuiCommand::CursorHome),
        (KeyModifiers::NONE, KeyCode::End) => Some(TuiCommand::CursorEnd),

        // Word movement (Ctrl+Arrow)
        (KeyModifiers::CONTROL, KeyCode::Left) => Some(TuiCommand::WordLeft),
        (KeyModifiers::CONTROL, KeyCode::Right) => Some(TuiCommand::WordRight),

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
        // Shift+Enter inserts newline (aligned with Claude Code behavior)
        let event = key(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::InsertNewline)
        );
    }

    #[test]
    fn test_alt_enter_inserts_newline() {
        // Alt+Enter inserts newline for multi-line input
        let event = key(KeyCode::Enter, KeyModifiers::ALT);
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

    #[test]
    fn test_ctrl_left_word_left() {
        let event = key(KeyCode::Left, KeyModifiers::CONTROL);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::WordLeft));
    }

    #[test]
    fn test_ctrl_right_word_right() {
        let event = key(KeyCode::Right, KeyModifiers::CONTROL);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::WordRight));
    }

    #[test]
    fn test_ctrl_backspace_delete_word() {
        let event = key(KeyCode::Backspace, KeyModifiers::CONTROL);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::DeleteWordBackward)
        );
    }

    #[test]
    fn test_ctrl_delete_delete_word_forward() {
        let event = key(KeyCode::Delete, KeyModifiers::CONTROL);
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::DeleteWordForward)
        );
    }

    #[test]
    fn test_f1_shows_help() {
        let event = key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::ShowHelp));
    }

    #[test]
    fn test_question_mark_shows_help() {
        let event = key(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(handle_key_event(event, false), Some(TuiCommand::ShowHelp));
    }

    #[test]
    fn test_ctrl_shift_t_toggles_thinking() {
        let event = key(
            KeyCode::Char('T'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            handle_key_event(event, false),
            Some(TuiCommand::ToggleThinking)
        );
    }

    // ========== Streaming-aware tests ==========

    #[test]
    fn test_enter_while_streaming_queues_input() {
        let event = key(KeyCode::Enter, KeyModifiers::NONE);
        // When streaming, Enter should queue instead of submit
        assert_eq!(
            handle_key_event_full(event, false, false, false, true),
            Some(TuiCommand::QueueInput)
        );
    }

    #[test]
    fn test_enter_while_not_streaming_submits() {
        let event = key(KeyCode::Enter, KeyModifiers::NONE);
        // When not streaming, Enter should submit
        assert_eq!(
            handle_key_event_full(event, false, false, false, false),
            Some(TuiCommand::SubmitInput)
        );
    }

    #[test]
    fn test_ctrl_enter_matches_enter_behavior() {
        let event = key(KeyCode::Enter, KeyModifiers::CONTROL);
        // Ctrl+Enter behaves the same as Enter: queue when streaming, submit otherwise
        assert_eq!(
            handle_key_event_full(event, false, false, false, true),
            Some(TuiCommand::QueueInput)
        );
        assert_eq!(
            handle_key_event_full(event, false, false, false, false),
            Some(TuiCommand::SubmitInput)
        );
    }

    #[test]
    fn test_shift_enter_inserts_newline_regardless_of_streaming() {
        let event = key(KeyCode::Enter, KeyModifiers::SHIFT);
        // Shift+Enter inserts newline regardless of streaming state
        assert_eq!(
            handle_key_event_full(event, false, false, false, true),
            Some(TuiCommand::InsertNewline)
        );
        assert_eq!(
            handle_key_event_full(event, false, false, false, false),
            Some(TuiCommand::InsertNewline)
        );
    }
}
