#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HookToolName {
    name: String,
    matcher_aliases: Vec<String>,
}

impl HookToolName {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            matcher_aliases: Vec::new(),
        }
    }

    pub(crate) fn apply_patch() -> Self {
        Self {
            name: "apply_patch".to_string(),
            matcher_aliases: vec!["Write".to_string(), "Edit".to_string()],
        }
    }

    pub(crate) fn bash() -> Self {
        Self::new("Bash")
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn matcher_aliases(&self) -> &[String] {
        &self.matcher_aliases
    }
}
