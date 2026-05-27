//! Hidden user-context fragment for runtime-owned goal steering prompts.

use super::ContextualUserFragment;

/// Hidden runtime-owned goal steering context injected into model input.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalContext {
    prompt: String,
}

impl GoalContext {
    /// Creates goal context around an already-rendered steering prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
        }
    }
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
