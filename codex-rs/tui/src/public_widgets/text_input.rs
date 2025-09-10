//! Public, minimal wrapper around the internal multiline TextArea widget.
//!
//! This exposes a stable, crate-agnostic text input for other Codex crates
//! (e.g., cloud-tasks) without making the whole TextArea API public.

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidgetRef;

// Use the internal text area implementation.
use crate::bottom_pane::textarea::TextArea as InnerTextArea;
use crate::bottom_pane::textarea::TextAreaState as InnerTextAreaState;

use std::cell::RefCell;

/// A reusable, multiline text input field with wrapping and cursor movement.
///
/// This wrapper intentionally exposes a very small surface area needed by
/// external consumers, while delegating to codex-tui's internal TextArea for
/// behavior and rendering.
pub struct TextInput {
    ta: InnerTextArea,
    state: RefCell<InnerTextAreaState>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    /// Create a new, empty input.
    pub fn new() -> Self {
        Self {
            ta: InnerTextArea::new(),
            state: RefCell::new(InnerTextAreaState::default()),
        }
    }

    /// Set the input contents.
    pub fn set_text(&mut self, text: &str) {
        self.ta.set_text(text);
    }

    /// Return the current text contents.
    pub fn text(&self) -> &str {
        self.ta.text()
    }

    /// Clear the input.
    pub fn clear(&mut self) {
        self.ta.set_text("");
    }

    /// Returns true if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.ta.is_empty()
    }

    /// Handle a key event (inserts characters, moves cursor, etc.).
    pub fn input(&mut self, key: KeyEvent) {
        self.ta.input(key);
    }

    /// Desired height (in rows) for a given width.
    pub fn desired_height(&self, width: u16) -> u16 {
        self.ta.desired_height(width)
    }

    /// Compute the on-screen cursor position for the given area.
    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let state = self.state.borrow();
        self.ta.cursor_pos_with_state(area, &state)
    }

    /// Render the input into the provided buffer at `area`.
    pub fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.ta), area, buf, &mut state);
    }
}
