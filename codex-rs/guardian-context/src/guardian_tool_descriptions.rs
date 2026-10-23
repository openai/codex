//! Bounded, untrusted descriptions for the exact MCP action under review.
//! Sync preparation applies per-source limits before rendering.

use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

/// Descriptions may be shortened or omitted without changing the required action.
#[derive(Clone, PartialEq)]
pub struct GuardianToolDescriptions {
    // Empty until sync preparation; prepared framing is always nonempty.
    pub(crate) text: String,
    pub(crate) tool: String,
    pub(crate) connector: String,
}

impl GuardianToolDescriptions {
    /// Captures descriptions without shortening them; sync preparation applies their caps.
    pub fn new(tool: Option<&str>, connector: Option<&str>) -> Option<Self> {
        if tool.is_none() && connector.is_none() {
            return None;
        }
        Some(Self {
            text: String::new(),
            tool: tool.unwrap_or_default().to_owned(),
            connector: connector.unwrap_or_default().to_owned(),
        })
    }
}

impl ContextualUserFragment for GuardianToolDescriptions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.tool_descriptions".to_owned())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "<guardian_tool_descriptions>",
            "</guardian_tool_descriptions>",
        )
    }

    fn body(&self) -> ContentItem {
        input_text(self.text.clone())
    }
}
