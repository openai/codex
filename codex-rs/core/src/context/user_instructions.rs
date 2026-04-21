use std::borrow::Cow;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserInstructions {
    pub(crate) directory: String,
    pub(crate) text: String,
}

impl ContextualUserFragment for UserInstructions {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "# AGENTS.md instructions for ";
    const END_MARKER: &'static str = "</INSTRUCTIONS>";
    const BODY_SEPARATOR: &'static str = "\n\n";

    fn start_marker_suffix(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.directory)
    }

    fn body(&self) -> String {
        format!("<INSTRUCTIONS>\n{}", self.text)
    }
}
