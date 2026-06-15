use super::ContextualUserFragment;

// This warning is not produced anymore but fragment definition is used to filter messaged from old sessions
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegacyUnifiedExecProcessLimitWarning;

impl ContextualUserFragment for LegacyUnifiedExecProcessLimitWarning {
    fn role(&self) -> &'static str {
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
            "Warning: The maximum number of unified exec processes you can keep open is",
        )
    }

    fn body(&self) -> String {
        String::new()
    }
}
