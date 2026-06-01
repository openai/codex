use super::ContextualUserFragment;

// This warning is not produced anymore but fragment definition is used to filter messaged from old sessions
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegacyModelMismatchWarning;

impl ContextualUserFragment for LegacyModelMismatchWarning {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn matches_text(text: &str) -> bool {
        text.trim().starts_with(
            "Warning: Your account was flagged for potentially high-risk cyber activity",
        )
    }

    fn body(&self) -> String {
        String::new()
    }
}
