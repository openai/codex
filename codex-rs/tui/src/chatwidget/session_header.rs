//! State for rendering the chat session header.
//!
//! The header currently only tracks the model label shown at the top of the chat view. The
//! state is intentionally small so it can be updated independently from the transcript layout.

/// Stores the model name shown in the chat header.
pub(crate) struct SessionHeader {
    /// Cached model label to avoid redundant allocations on repeated updates.
    model: String,
}

impl SessionHeader {
    /// Create a header initialized with the current model label.
    pub(crate) fn new(model: String) -> Self {
        Self { model }
    }

    /// Updates the header's model label when it changes.
    pub(crate) fn set_model(&mut self, model: &str) {
        if self.model != model {
            self.model = model.to_string();
        }
    }
}
