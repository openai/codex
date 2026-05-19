use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HookAdditionalContext {
    text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StickyHookAdditionalContext {
    text: String,
}

impl HookAdditionalContext {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl StickyHookAdditionalContext {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl ContextualUserFragment for HookAdditionalContext {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<hook_context>";
    const END_MARKER: &'static str = "</hook_context>";

    fn body(&self) -> String {
        self.text.clone()
    }
}

impl ContextualUserFragment for StickyHookAdditionalContext {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<hook_context_sticky>";
    const END_MARKER: &'static str = "</hook_context_sticky>";

    fn body(&self) -> String {
        self.text.clone()
    }
}
