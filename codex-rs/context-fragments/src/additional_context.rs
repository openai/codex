use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use codex_utils_string::truncate_middle_with_token_budget;

use crate::ContextualUserFragment;

const MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS: usize = 1_000;
const MAX_DERIVED_TOOL_NAME_CHARS: usize = 128;
const ADDITIONAL_CONTEXT_TOOL_NAMESPACE: &str = "agent";
const ADDITIONAL_CONTEXT_END_MARKER_SUFFIX: &str = ">";
const ADDITIONAL_CONTEXT_START_MARKER_PREFIX: &str = "<external_";

/// A named context output whose body is bounded before comparison or model input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalContextToolOutput {
    name: String,
    value: String,
}

impl AdditionalContextToolOutput {
    /// An explicit name wins; otherwise derive a label from the opaque source key.
    pub fn new(key: &str, name: Option<String>, value: &str) -> Self {
        let name = name.unwrap_or_else(|| tool_name_from_key(key));
        Self {
            name,
            value: truncate_middle_with_token_budget(value, MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS).0,
        }
    }
}

impl From<AdditionalContextToolOutput> for ResponseItem {
    fn from(output: AdditionalContextToolOutput) -> Self {
        Self::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some(output.name),
            namespace: Some(ADDITIONAL_CONTEXT_TOOL_NAMESPACE.to_owned()),
            output: FunctionCallOutputPayload::from_text(output.value),
            internal_chat_message_metadata_passthrough: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalContextUserFragment {
    key: String,
    value: String,
}

impl AdditionalContextUserFragment {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl ContextualUserFragment for AdditionalContextUserFragment {
    fn role(&self) -> &'static str {
        "user"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(format!("additional_content.{}", self.key))
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

    fn body(&self) -> ContentItem {
        input_text(additional_context_body(&self.key, &self.value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalContextDeveloperFragment {
    key: String,
    value: String,
}

impl AdditionalContextDeveloperFragment {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl ContextualUserFragment for AdditionalContextDeveloperFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(format!("additional_content.{}", self.key))
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(additional_context_developer_body(&self.key, &self.value))
    }
}

fn tool_name_from_key(key: &str) -> String {
    let derived: String = key
        .chars()
        .map(|ch| if ch == ' ' { '_' } else { ch })
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .take(MAX_DERIVED_TOOL_NAME_CHARS)
        .collect();
    if derived.is_empty() {
        "untrusted_content".to_owned()
    } else {
        derived
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
