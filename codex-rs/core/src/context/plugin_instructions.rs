use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PluginInstructions {
    text: String,
}

impl PluginInstructions {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl ContextualUserFragment for PluginInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn body(&self) -> String {
        self.text.clone()
    }
}
