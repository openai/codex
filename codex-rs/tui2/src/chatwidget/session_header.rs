pub(crate) struct SessionHeader {
    model: String,
}

impl SessionHeader {
    pub(crate) fn new(model: String) -> Self {
        Self { model }
    }
// tui2/src/chatwidget/session_header.rs

    /// Updates the header's model text.
    pub(crate) fn set_model(&mut self, model: &str) {
        if self.model != model {
            self.model = model.to_string();
        }
    }
}
