use super::ContextualUserFragment;

pub use codex_prompts::PermissionsInstructions;

impl ContextualUserFragment for PermissionsInstructions {
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<permissions instructions>", "</permissions instructions>")
    }

    fn body(&self) -> String {
        PermissionsInstructions::body(self)
    }
}
