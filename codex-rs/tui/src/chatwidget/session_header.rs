use std::path::PathBuf;

pub(crate) struct SessionHeader {
    model: String,
}

impl SessionHeader {
    // Keep signature to avoid changing call sites; underscore params silence warnings.
    pub(crate) fn new(model: String, _directory: PathBuf, _version: &'static str) -> Self {
        Self { model }
    }

    pub(crate) fn set_model(&mut self, model: String) -> bool {
        if self.model == model {
            false
        } else {
            self.model = model;
            true
        }
    }
}
