use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionContext {
    body: String,
}

impl ExtensionContext {
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl ContextualUserFragment for ExtensionContext {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<extension_context>", "</extension_context>")
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.body)
    }
}
