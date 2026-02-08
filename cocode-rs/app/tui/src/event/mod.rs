//! TUI event handling.
//!
//! This module provides:
//! - [`TuiEvent`]: All events the TUI can receive (keyboard, mouse, resize, agent, etc.)
//! - [`TuiCommand`]: User commands triggered by keyboard shortcuts
//! - [`EventBroker`]: Controls stdin reading (pause/resume for external editors)
//! - [`EventStream`]: Async stream of TUI events

mod broker;
mod handler;
mod stream;

pub use broker::EventBroker;
pub use handler::handle_key_event;
pub use handler::handle_key_event_full;
pub use handler::handle_key_event_with_suggestions;
pub use stream::TuiEventStream;

use cocode_protocol::LoopEvent;
use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;

/// Events that can be processed by the TUI.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    // ========== Terminal Events ==========
    /// A key was pressed.
    Key(KeyEvent),

    /// A mouse event occurred.
    Mouse(MouseEvent),

    /// The terminal was resized.
    Resize {
        /// New terminal width.
        width: u16,
        /// New terminal height.
        height: u16,
    },

    /// Focus changed (terminal gained/lost focus).
    FocusChanged {
        /// Whether the terminal is now focused.
        focused: bool,
    },

    // ========== Internal Events ==========
    /// Request to draw a frame.
    Draw,

    /// Periodic tick for animations and status updates.
    Tick,

    /// Paste event (bracketed paste mode).
    Paste(String),

    // ========== Agent Events ==========
    /// Event from the core agent loop.
    Agent(LoopEvent),

    // ========== Commands ==========
    /// A user command to execute.
    Command(TuiCommand),
}

/// Commands that can be triggered by the user.
///
/// These represent high-level actions that modify application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiCommand {
    // ========== Mode Toggles ==========
    /// Toggle plan mode on/off.
    TogglePlanMode,

    /// Cycle through thinking levels (None → Low → Medium → High → XHigh → None).
    CycleThinkingLevel,

    /// Switch to a different model.
    CycleModel,

    /// Show model picker overlay.
    ShowModelPicker,

    // ========== Input Actions ==========
    /// Submit the current input.
    SubmitInput,

    /// Interrupt the current operation.
    Interrupt,

    /// Clear the screen and redraw.
    ClearScreen,

    /// Cancel current input or close overlay.
    Cancel,

    // ========== Navigation ==========
    /// Scroll up in the chat history.
    ScrollUp,

    /// Scroll down in the chat history.
    ScrollDown,

    /// Page up in the chat history.
    PageUp,

    /// Page down in the chat history.
    PageDown,

    /// Move focus to the next element.
    FocusNext,

    /// Move focus to the previous element.
    FocusPrevious,

    // ========== File Autocomplete ==========
    /// Select next file suggestion.
    SelectNextSuggestion,

    /// Select previous file suggestion.
    SelectPrevSuggestion,

    /// Accept the current file suggestion.
    AcceptSuggestion,

    /// Dismiss file suggestions.
    DismissSuggestions,

    // ========== Skill Autocomplete ==========
    /// Select next skill suggestion.
    SelectNextSkillSuggestion,

    /// Select previous skill suggestion.
    SelectPrevSkillSuggestion,

    /// Accept the current skill suggestion.
    AcceptSkillSuggestion,

    /// Dismiss skill suggestions.
    DismissSkillSuggestions,

    // ========== Editing ==========
    /// Insert a character at the cursor.
    InsertChar(char),

    /// Delete the character before the cursor.
    DeleteBackward,

    /// Delete the character at the cursor.
    DeleteForward,

    /// Move cursor left.
    CursorLeft,

    /// Move cursor right.
    CursorRight,

    /// Move cursor up (in multi-line input).
    CursorUp,

    /// Move cursor down (in multi-line input).
    CursorDown,

    /// Move cursor to start of line.
    CursorHome,

    /// Move cursor to end of line.
    CursorEnd,

    /// Move cursor to the start of the previous word.
    WordLeft,

    /// Move cursor to the start of the next word.
    WordRight,

    /// Delete the word before the cursor.
    DeleteWordBackward,

    /// Delete the word after the cursor.
    DeleteWordForward,

    /// Insert a newline in the input.
    InsertNewline,

    // ========== Approval ==========
    /// Approve the current permission request.
    Approve,

    /// Deny the current permission request.
    Deny,

    /// Approve all similar permission requests.
    ApproveAll,

    // ========== External Editor ==========
    /// Open the current input in an external editor.
    OpenExternalEditor,

    // ========== Help ==========
    /// Show the help overlay.
    ShowHelp,

    // ========== Command Palette ==========
    /// Show command palette.
    ShowCommandPalette,

    // ========== Session Browser ==========
    /// Show session browser.
    ShowSessionBrowser,

    /// Load a session.
    LoadSession(String),

    /// Delete a session.
    DeleteSession(String),

    // ========== Thinking Toggle ==========
    /// Toggle display of thinking content in chat.
    ToggleThinking,

    // ========== Clipboard Paste ==========
    /// Smart paste from clipboard: try image first, fall back to text.
    PasteFromClipboard,

    // ========== Queue ==========
    /// Queue input for steering injection (Enter while streaming).
    ///
    /// The command is consumed once in the agent loop and injected as a
    /// steering system-reminder that asks the model to address the message.
    QueueInput,

    // ========== Quit ==========
    /// Request to quit the application.
    Quit,
}

impl std::fmt::Display for TuiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::i18n::t;

        match self {
            TuiCommand::TogglePlanMode => write!(f, "{}", t!("command.toggle_plan_mode")),
            TuiCommand::CycleThinkingLevel => write!(f, "{}", t!("command.cycle_thinking_level")),
            TuiCommand::CycleModel => write!(f, "{}", t!("command.cycle_model")),
            TuiCommand::ShowModelPicker => write!(f, "{}", t!("command.show_model_picker")),
            TuiCommand::SelectNextSuggestion => {
                write!(f, "{}", t!("command.select_next_suggestion"))
            }
            TuiCommand::SelectPrevSuggestion => {
                write!(f, "{}", t!("command.select_prev_suggestion"))
            }
            TuiCommand::AcceptSuggestion => write!(f, "{}", t!("command.accept_suggestion")),
            TuiCommand::DismissSuggestions => write!(f, "{}", t!("command.dismiss_suggestions")),
            TuiCommand::SelectNextSkillSuggestion => {
                write!(f, "{}", t!("command.select_next_skill_suggestion"))
            }
            TuiCommand::SelectPrevSkillSuggestion => {
                write!(f, "{}", t!("command.select_prev_skill_suggestion"))
            }
            TuiCommand::AcceptSkillSuggestion => {
                write!(f, "{}", t!("command.accept_skill_suggestion"))
            }
            TuiCommand::DismissSkillSuggestions => {
                write!(f, "{}", t!("command.dismiss_skill_suggestions"))
            }
            TuiCommand::SubmitInput => write!(f, "{}", t!("command.submit_input")),
            TuiCommand::Interrupt => write!(f, "{}", t!("command.interrupt")),
            TuiCommand::ClearScreen => write!(f, "{}", t!("command.clear_screen")),
            TuiCommand::Cancel => write!(f, "{}", t!("command.cancel")),
            TuiCommand::ScrollUp => write!(f, "{}", t!("command.scroll_up")),
            TuiCommand::ScrollDown => write!(f, "{}", t!("command.scroll_down")),
            TuiCommand::PageUp => write!(f, "{}", t!("command.page_up")),
            TuiCommand::PageDown => write!(f, "{}", t!("command.page_down")),
            TuiCommand::FocusNext => write!(f, "{}", t!("command.focus_next")),
            TuiCommand::FocusPrevious => write!(f, "{}", t!("command.focus_previous")),
            TuiCommand::InsertChar(c) => write!(f, "{}", t!("command.insert_char", c = c)),
            TuiCommand::DeleteBackward => write!(f, "{}", t!("command.delete_backward")),
            TuiCommand::DeleteForward => write!(f, "{}", t!("command.delete_forward")),
            TuiCommand::CursorLeft => write!(f, "{}", t!("command.cursor_left")),
            TuiCommand::CursorRight => write!(f, "{}", t!("command.cursor_right")),
            TuiCommand::CursorUp => write!(f, "{}", t!("command.cursor_up")),
            TuiCommand::CursorDown => write!(f, "{}", t!("command.cursor_down")),
            TuiCommand::CursorHome => write!(f, "{}", t!("command.cursor_home")),
            TuiCommand::CursorEnd => write!(f, "{}", t!("command.cursor_end")),
            TuiCommand::WordLeft => write!(f, "{}", t!("command.word_left")),
            TuiCommand::WordRight => write!(f, "{}", t!("command.word_right")),
            TuiCommand::DeleteWordBackward => write!(f, "{}", t!("command.delete_word_backward")),
            TuiCommand::DeleteWordForward => write!(f, "{}", t!("command.delete_word_forward")),
            TuiCommand::InsertNewline => write!(f, "{}", t!("command.insert_newline")),
            TuiCommand::Approve => write!(f, "{}", t!("command.approve")),
            TuiCommand::Deny => write!(f, "{}", t!("command.deny")),
            TuiCommand::ApproveAll => write!(f, "{}", t!("command.approve_all")),
            TuiCommand::OpenExternalEditor => write!(f, "{}", t!("command.open_external_editor")),
            TuiCommand::ShowHelp => write!(f, "{}", t!("command.show_help")),
            TuiCommand::ShowCommandPalette => write!(f, "{}", t!("command.show_command_palette")),
            TuiCommand::ShowSessionBrowser => write!(f, "{}", t!("command.show_session_browser")),
            TuiCommand::LoadSession(id) => write!(f, "{}", t!("command.load_session", id = id)),
            TuiCommand::DeleteSession(id) => write!(f, "{}", t!("command.delete_session", id = id)),
            TuiCommand::ToggleThinking => write!(f, "{}", t!("command.toggle_thinking")),
            TuiCommand::PasteFromClipboard => {
                write!(f, "{}", t!("command.paste_from_clipboard"))
            }
            TuiCommand::QueueInput => write!(f, "{}", t!("command.queue_input")),
            TuiCommand::Quit => write!(f, "{}", t!("command.quit")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_command_display() {
        // Test that Display impl produces non-empty translated strings
        assert!(!TuiCommand::TogglePlanMode.to_string().is_empty());
        assert!(!TuiCommand::InsertChar('a').to_string().is_empty());
        assert!(!TuiCommand::Quit.to_string().is_empty());
        assert!(!TuiCommand::WordLeft.to_string().is_empty());
        assert!(!TuiCommand::WordRight.to_string().is_empty());
        assert!(!TuiCommand::DeleteWordBackward.to_string().is_empty());
        assert!(!TuiCommand::DeleteWordForward.to_string().is_empty());
        assert!(!TuiCommand::ShowHelp.to_string().is_empty());
        assert!(!TuiCommand::ToggleThinking.to_string().is_empty());
    }

    #[test]
    fn test_tui_event_variants() {
        // Verify we can create all event variants
        let _key = TuiEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let _resize = TuiEvent::Resize {
            width: 80,
            height: 24,
        };
        let _draw = TuiEvent::Draw;
        let _tick = TuiEvent::Tick;
        let _paste = TuiEvent::Paste("test".to_string());
        let _command = TuiEvent::Command(TuiCommand::Quit);
    }
}
