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

    // ========== Quit ==========
    /// Request to quit the application.
    Quit,
}

impl std::fmt::Display for TuiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TuiCommand::TogglePlanMode => write!(f, "Toggle Plan Mode"),
            TuiCommand::CycleThinkingLevel => write!(f, "Cycle Thinking Level"),
            TuiCommand::CycleModel => write!(f, "Cycle Model"),
            TuiCommand::ShowModelPicker => write!(f, "Show Model Picker"),
            TuiCommand::SubmitInput => write!(f, "Submit Input"),
            TuiCommand::Interrupt => write!(f, "Interrupt"),
            TuiCommand::ClearScreen => write!(f, "Clear Screen"),
            TuiCommand::Cancel => write!(f, "Cancel"),
            TuiCommand::ScrollUp => write!(f, "Scroll Up"),
            TuiCommand::ScrollDown => write!(f, "Scroll Down"),
            TuiCommand::PageUp => write!(f, "Page Up"),
            TuiCommand::PageDown => write!(f, "Page Down"),
            TuiCommand::FocusNext => write!(f, "Focus Next"),
            TuiCommand::FocusPrevious => write!(f, "Focus Previous"),
            TuiCommand::InsertChar(c) => write!(f, "Insert '{c}'"),
            TuiCommand::DeleteBackward => write!(f, "Delete Backward"),
            TuiCommand::DeleteForward => write!(f, "Delete Forward"),
            TuiCommand::CursorLeft => write!(f, "Cursor Left"),
            TuiCommand::CursorRight => write!(f, "Cursor Right"),
            TuiCommand::CursorUp => write!(f, "Cursor Up"),
            TuiCommand::CursorDown => write!(f, "Cursor Down"),
            TuiCommand::CursorHome => write!(f, "Cursor Home"),
            TuiCommand::CursorEnd => write!(f, "Cursor End"),
            TuiCommand::InsertNewline => write!(f, "Insert Newline"),
            TuiCommand::Approve => write!(f, "Approve"),
            TuiCommand::Deny => write!(f, "Deny"),
            TuiCommand::ApproveAll => write!(f, "Approve All"),
            TuiCommand::OpenExternalEditor => write!(f, "Open External Editor"),
            TuiCommand::Quit => write!(f, "Quit"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_command_display() {
        assert_eq!(TuiCommand::TogglePlanMode.to_string(), "Toggle Plan Mode");
        assert_eq!(TuiCommand::InsertChar('a').to_string(), "Insert 'a'");
        assert_eq!(TuiCommand::Quit.to_string(), "Quit");
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
