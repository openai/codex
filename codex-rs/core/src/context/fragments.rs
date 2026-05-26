use super::ContextualUserFragment;
use codex_utils_string::truncate_middle_with_token_budget;

const MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS: usize = 1_000;
const ADDITIONAL_CONTEXT_END_MARKER_SUFFIX: &str = ">";
const ADDITIONAL_CONTEXT_START_MARKER_PREFIX: &str = "<external_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdditionalContextUserFragment {
    key: String,
    value: String,
}

impl AdditionalContextUserFragment {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl ContextualUserFragment for AdditionalContextUserFragment {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            ADDITIONAL_CONTEXT_START_MARKER_PREFIX,
            ADDITIONAL_CONTEXT_END_MARKER_SUFFIX,
        )
    }

    fn matches_text(text: &str) -> bool {
        let trimmed = text.trim();
        let Some(rest) = trimmed.strip_prefix(ADDITIONAL_CONTEXT_START_MARKER_PREFIX) else {
            return false;
        };
        let Some((key, value_and_close)) = rest.split_once(ADDITIONAL_CONTEXT_END_MARKER_SUFFIX)
        else {
            return false;
        };

        value_and_close.ends_with(&format!("</external_{key}>"))
    }

    fn body(&self) -> String {
        additional_context_body(&self.key, &self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdditionalContextDeveloperFragment {
    key: String,
    value: String,
}

impl AdditionalContextDeveloperFragment {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl ContextualUserFragment for AdditionalContextDeveloperFragment {
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
        additional_context_developer_body(&self.key, &self.value)
    }
}

fn additional_context_body(key: &str, value: &str) -> String {
    let value = truncate_middle_with_token_budget(value, MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS).0;
    format!("{key}>{value}</external_{key}")
}

fn additional_context_developer_body(key: &str, value: &str) -> String {
    let value = truncate_middle_with_token_budget(value, MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS).0;
    format!("<{key}>{value}</{key}>")
}
