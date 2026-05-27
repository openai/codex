//! Hidden user-context wrappers owned by extensions.
//!
//! Extensions can use this host-recognized tag when they need hidden
//! model-visible context to survive history filtering.

use super::ContextualUserFragment;
use super::fragment::matches_marked_text;

const START_MARKER: &str = "<extension_context>";
const END_MARKER: &str = "</extension_context>";

/// Hidden user-context fragment for extension-owned steering prompts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionContext {
    body: String,
}

impl ExtensionContext {
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl ContextualUserFragment for ExtensionContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        (START_MARKER, END_MARKER)
    }

    fn type_markers() -> (&'static str, &'static str) {
        (START_MARKER, END_MARKER)
    }

    fn matches_text(text: &str) -> bool {
        matches_marked_text(START_MARKER, END_MARKER, text)
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.body)
    }
}
