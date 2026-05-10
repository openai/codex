//! Hidden user-context fragment for runtime-owned goal steering prompts.

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GoalContext {
    pub(crate) prompt: String,
}

impl ContextualUserFragment for GoalContext {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "<goal_context>";
    const END_MARKER: &'static str = "</goal_context>";

    fn body(&self) -> String {
        format!("\n{}\n", self.prompt)
    }
}
