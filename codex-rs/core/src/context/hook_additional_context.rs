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
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<hook_context>", "</hook_context>")
    }

    fn body(&self) -> String {
        self.text.clone()
    }
}

impl ContextualUserFragment for StickyHookAdditionalContext {
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<hook_context_sticky>", "</hook_context_sticky>")
    }

    fn body(&self) -> String {
        self.text.clone()
    }
}
