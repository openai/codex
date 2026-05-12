use super::ContextualUserFragment;

// This warning is not produced anymore but fragment definition is used to filter messaged from old sessions
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegacyUnifiedExecProcessLimitWarning;

impl ContextualUserFragment for LegacyUnifiedExecProcessLimitWarning {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn matches_text(text: &str) -> bool {
        text.trim().starts_with(
            "Warning: The maximum number of unified exec processes you can keep open is",
        )
    }

    fn body(&self) -> String {
        String::new()
    }
}
