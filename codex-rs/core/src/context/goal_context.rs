//! Legacy hidden user-context marker for goal steering prompts.
//!
//! The goal implementation now lives in `codex-goal-extension`, but existing
//! rollouts can still contain this marker. Core keeps recognizing it so old
//! hidden goal prompts do not resurface as normal user messages after resume.

use super::ContextualUserFragment;

/// Marker-only registration for hidden goal steering context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalContext;

impl ContextualUserFragment for GoalContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn body(&self) -> String {
        String::new()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<goal_context>", "</goal_context>")
    }
}
