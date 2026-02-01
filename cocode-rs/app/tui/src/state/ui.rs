//! UI-specific state.
//!
//! This module contains state that is local to the TUI and not
//! synchronized with the agent.

use cocode_protocol::ApprovalRequest;

/// UI-specific state.
#[derive(Debug, Clone, Default)]
pub struct UiState {
    /// The current input state.
    pub input: InputState,

    /// Scroll offset in the chat history.
    pub scroll_offset: i32,

    /// Current focus target.
    pub focus: FocusTarget,

    /// Active overlay (modal dialog).
    pub overlay: Option<Overlay>,

    /// Streaming content state.
    pub streaming: Option<StreamingState>,
}

impl UiState {
    /// Set the overlay.
    pub fn set_overlay(&mut self, overlay: Overlay) {
        self.overlay = Some(overlay);
    }

    /// Clear the overlay.
    pub fn clear_overlay(&mut self) {
        self.overlay = None;
    }

    /// Start streaming.
    pub fn start_streaming(&mut self, turn_id: String) {
        self.streaming = Some(StreamingState::new(turn_id));
    }

    /// Stop streaming.
    pub fn stop_streaming(&mut self) {
        self.streaming = None;
    }

    /// Append to streaming content.
    pub fn append_streaming(&mut self, delta: &str) {
        if let Some(ref mut streaming) = self.streaming {
            streaming.content.push_str(delta);
        }
    }

    /// Append to streaming thinking content.
    pub fn append_streaming_thinking(&mut self, delta: &str) {
        if let Some(ref mut streaming) = self.streaming {
            streaming.thinking.push_str(delta);
        }
    }
}

/// State for the input field.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// The current input text.
    pub text: String,

    /// Cursor position (character index).
    pub cursor: i32,

    /// Selection start (if any).
    pub selection_start: Option<i32>,

    /// History of previous inputs.
    pub history: Vec<String>,

    /// Current history index (for up/down navigation).
    pub history_index: Option<i32>,
}

impl InputState {
    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        let cursor = self.cursor as usize;
        if cursor >= self.text.len() {
            self.text.push(c);
        } else {
            self.text.insert(cursor, c);
        }
        self.cursor += 1;
    }

    /// Delete the character before the cursor.
    pub fn delete_backward(&mut self) {
        if self.cursor > 0 {
            let cursor = (self.cursor - 1) as usize;
            if cursor < self.text.len() {
                self.text.remove(cursor);
            }
            self.cursor -= 1;
        }
    }

    /// Delete the character at the cursor.
    pub fn delete_forward(&mut self) {
        let cursor = self.cursor as usize;
        if cursor < self.text.len() {
            self.text.remove(cursor);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if (self.cursor as usize) < self.text.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn move_end(&mut self) {
        self.cursor = self.text.len() as i32;
    }

    /// Insert a newline.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Clear the input and return the text.
    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.selection_start = None;
        text
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set the text (e.g., from history or paste).
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len() as i32;
    }
}

/// The current focus target in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// Input field is focused.
    #[default]
    Input,
    /// Chat history is focused (for scrolling).
    Chat,
    /// Tool panel is focused.
    ToolPanel,
}

/// An active overlay (modal dialog).
#[derive(Debug, Clone)]
pub enum Overlay {
    /// Permission approval prompt.
    Permission(PermissionOverlay),
    /// Model picker.
    ModelPicker(ModelPickerOverlay),
    /// Help screen.
    Help,
    /// Error message.
    Error(String),
}

/// Permission approval overlay state.
#[derive(Debug, Clone)]
pub struct PermissionOverlay {
    /// The approval request.
    pub request: ApprovalRequest,
    /// Selected option index (0 = approve, 1 = deny, 2 = approve all).
    pub selected: i32,
}

impl PermissionOverlay {
    /// Create a new permission overlay.
    pub fn new(request: ApprovalRequest) -> Self {
        Self {
            request,
            selected: 0,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if self.selected < 2 {
            self.selected += 1;
        }
    }
}

/// Model picker overlay state.
#[derive(Debug, Clone)]
pub struct ModelPickerOverlay {
    /// Available models.
    pub models: Vec<String>,
    /// Currently selected index.
    pub selected: i32,
    /// Search filter.
    pub filter: String,
}

impl ModelPickerOverlay {
    /// Create a new model picker.
    pub fn new(models: Vec<String>) -> Self {
        Self {
            models,
            selected: 0,
            filter: String::new(),
        }
    }

    /// Get filtered models.
    pub fn filtered_models(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.models.iter().map(String::as_str).collect()
        } else {
            let filter = self.filter.to_lowercase();
            self.models
                .iter()
                .filter(|m| m.to_lowercase().contains(&filter))
                .map(String::as_str)
                .collect()
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = self.filtered_models().len() as i32 - 1;
        if self.selected < max {
            self.selected += 1;
        }
    }
}

/// State for streaming content.
#[derive(Debug, Clone)]
pub struct StreamingState {
    /// Turn identifier.
    pub turn_id: String,
    /// Content being streamed.
    pub content: String,
    /// Thinking content being streamed.
    pub thinking: String,
}

impl StreamingState {
    /// Create new streaming state.
    pub fn new(turn_id: String) -> Self {
        Self {
            turn_id,
            content: String::new(),
            thinking: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_insert() {
        let mut input = InputState::default();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.text(), "Hi");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_input_state_delete() {
        let mut input = InputState::default();
        input.set_text("Hello");
        input.cursor = 3; // After "Hel"

        input.delete_backward();
        assert_eq!(input.text(), "Helo");
        assert_eq!(input.cursor, 2);

        input.delete_forward();
        assert_eq!(input.text(), "Heo");
    }

    #[test]
    fn test_input_state_navigation() {
        let mut input = InputState::default();
        input.set_text("Hello");

        input.move_home();
        assert_eq!(input.cursor, 0);

        input.move_right();
        assert_eq!(input.cursor, 1);

        input.move_end();
        assert_eq!(input.cursor, 5);

        input.move_left();
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_input_state_take() {
        let mut input = InputState::default();
        input.set_text("Hello");

        let text = input.take();
        assert_eq!(text, "Hello");
        assert!(input.is_empty());
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_streaming_state() {
        let mut ui = UiState::default();

        ui.start_streaming("turn-1".to_string());
        assert!(ui.streaming.is_some());

        ui.append_streaming("Hello ");
        ui.append_streaming("World");
        assert_eq!(
            ui.streaming.as_ref().map(|s| s.content.as_str()),
            Some("Hello World")
        );

        ui.stop_streaming();
        assert!(ui.streaming.is_none());
    }

    #[test]
    fn test_focus_target_default() {
        assert_eq!(FocusTarget::default(), FocusTarget::Input);
    }
}
