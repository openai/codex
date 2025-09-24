pub(crate) struct SessionHeader {
    model: String,
    thread_path: Vec<String>,
}

impl SessionHeader {
    pub(crate) fn new(model: String) -> Self {
        Self {
            model,
            thread_path: vec!["main".to_string()],
        }
    }

    /// Updates the header's model text.
    pub(crate) fn set_model(&mut self, model: &str) {
        if self.model != model {
            self.model = model.to_string();
        }
    }

    pub(crate) fn set_thread_path(&mut self, parts: Vec<String>) {
        if !parts.is_empty() {
            self.thread_path = parts;
        }
    }

    pub(crate) fn thread_path(&self) -> &[String] {
        &self.thread_path
    }
}
