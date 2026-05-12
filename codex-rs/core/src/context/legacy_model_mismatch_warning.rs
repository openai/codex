use super::ContextualUserFragment;

// This warning is not produced anymore but fragment definition is used to filter messaged from old sessions
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegacyModelMismatchWarning;

impl ContextualUserFragment for LegacyModelMismatchWarning {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn matches_text(text: &str) -> bool {
        text.trim().starts_with(
            "Warning: Your account was flagged for potentially high-risk cyber activity",
        )
    }

    fn body(&self) -> String {
        String::new()
    }
}
