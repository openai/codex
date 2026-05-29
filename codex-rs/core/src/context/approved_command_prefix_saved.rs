use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ApprovedCommandPrefixSaved {
    prefixes: String,
}

impl ApprovedCommandPrefixSaved {
    pub(crate) fn new(prefixes: impl Into<String>) -> Self {
        Self {
            prefixes: prefixes.into(),
        }
    }
}

impl ContextualUserFragment for ApprovedCommandPrefixSaved {
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        format!("Approved command prefix saved:\n{}", self.prefixes)
    }
}
