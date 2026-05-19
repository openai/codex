use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserInstructions {
    pub(crate) directory: String,
    pub(crate) text: String,
}

impl ContextualUserFragment for UserInstructions {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("# AGENTS.md instructions for ", "</INSTRUCTIONS>")
    }

    fn body(&self) -> String {
        format!("{}\n\n<INSTRUCTIONS>\n{}\n", self.directory, self.text)
    }
}
