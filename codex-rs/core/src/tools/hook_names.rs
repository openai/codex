//! Hook-facing tool names and matcher compatibility aliases.
//!
//! Hook stdin exposes one canonical `tool_name`, but matcher selection may also
//! need to recognize names from adjacent tool ecosystems. Keeping those two
//! concepts together prevents handlers from accidentally serializing a
//! compatibility alias, such as `Write`, as the stable hook payload name.

/// Identifies a tool in hook payloads and hook matcher selection.
///
/// `name` is the canonical value serialized into hook stdin. Matcher aliases are
/// internal-only compatibility names that may select the same hook handlers but
/// must not change the payload seen by hook processes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HookToolName {
    name: String,
    matcher_aliases: Vec<String>,
}

impl HookToolName {
    /// Builds a hook tool name with no matcher aliases.
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            matcher_aliases: Vec::new(),
        }
    }

    /// Returns the hook identity for file edits performed through `apply_patch`.
    ///
    /// The serialized name remains `apply_patch` so logs and policies can key
    /// off the actual Codex tool. `Write` and `Edit` are accepted as matcher
    /// aliases for compatibility with hook configurations that describe edits
    /// using Claude Code-style names.
    pub(crate) fn apply_patch() -> Self {
        Self {
            name: "apply_patch".to_string(),
            matcher_aliases: vec!["Write".to_string(), "Edit".to_string()],
        }
    }

    /// Returns the hook identity historically used for shell-like tools.
    pub(crate) fn bash() -> Self {
        Self::new("Bash")
    }

    /// Returns the canonical hook name serialized into hook stdin.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns additional matcher inputs that should select the same handlers.
    pub(crate) fn matcher_aliases(&self) -> &[String] {
        &self.matcher_aliases
    }
}
