use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeveloperInstructions {
    instructions: String,
}

impl DeveloperInstructions {
    pub(crate) fn new(instructions: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
        }
    }
}

impl ContextualUserFragment for DeveloperInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("generic.developer_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.instructions.clone()
    }
}
