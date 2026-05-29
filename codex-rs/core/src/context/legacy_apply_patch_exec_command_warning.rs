use super::ContextualUserFragment;

// This warning is not produced anymore but fragment definition is used to filter messaged from old sessions
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LegacyApplyPatchExecCommandWarning;

impl ContextualUserFragment for LegacyApplyPatchExecCommandWarning {
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
        let trimmed = text.trim();
        trimmed.starts_with("Warning: apply_patch was requested via ")
            && trimmed.ends_with("Use the apply_patch tool instead of exec_command.")
    }

    fn body(&self) -> String {
        String::new()
    }
}
