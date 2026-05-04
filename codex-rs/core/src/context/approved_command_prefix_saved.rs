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
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn body(&self) -> String {
        format!("Approved command prefix saved:\n{}", self.prefixes)
    }
}
