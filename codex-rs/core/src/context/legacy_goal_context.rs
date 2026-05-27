use super::ContextualUserFragment;

/// Legacy hidden goal-context fragment.
///
/// Goal prompts are now owned by the goal extension and wrapped as extension
/// context. This keeps older hidden goal prompts from being surfaced as visible
/// user text when stored conversation history is replayed.
pub(crate) struct LegacyGoalContext;

impl ContextualUserFragment for LegacyGoalContext {
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
        String::new()
    }
}
