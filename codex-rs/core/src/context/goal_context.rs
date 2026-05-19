//! Hidden user-context fragment for runtime-owned goal steering prompts.

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GoalContext {
    pub(crate) prompt: String,
}

impl ContextualUserFragment for GoalContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<goal_context>", "</goal_context>")
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.prompt)
    }
}
