//! Resolves incremental catalog wording without changing catalog identity or update timing.
//! Text is owned because world-state sections outlive the borrowed model metadata.

use codex_protocol::openai_models::IncrementalToolMessages;

/// Resolved static wording; consumers retain ownership of runtime names and instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIncrementalToolMessages {
    pub tool_update_hint: String,
    pub removed_tools_header: String,
    pub removed_namespaces_header: String,
    pub namespace_instructions_prefix: String,
    pub namespace_instructions_cleared: String,
}

impl Default for ResolvedIncrementalToolMessages {
    fn default() -> Self {
        Self::new(/*messages*/ None)
    }
}

impl ResolvedIncrementalToolMessages {
    pub(crate) fn new(messages: Option<&IncrementalToolMessages>) -> Self {
        let resolve = |text: Option<&String>, bundled: &str| {
            text.filter(|text| text.len() <= 512)
                .map_or_else(|| bundled.to_owned(), Clone::clone)
        };
        Self {
            tool_update_hint: resolve(
                messages.and_then(|m| m.tool_update_hint.as_ref()),
                "This is an incremental tools update. Previously declared tools remain available for direct calls unless explicitly marked unavailable. If a tool is redefined here, its latest definition replaces the earlier one.",
            ),
            removed_tools_header: resolve(
                messages.and_then(|m| m.removed_tools_header.as_ref()),
                "The following tools are no longer available. Do not call them:",
            ),
            removed_namespaces_header: resolve(
                messages.and_then(|m| m.removed_namespaces_header.as_ref()),
                "The following namespaces are no longer available. Do not call tools in them unless those tools are declared in a later update:",
            ),
            namespace_instructions_prefix: resolve(
                messages.and_then(|m| m.namespace_instructions_prefix.as_ref()),
                "Updated instructions for the {name} namespace:\n",
            ),
            namespace_instructions_cleared: resolve(
                messages.and_then(|m| m.namespace_instructions_cleared.as_ref()),
                "The {name} namespace no longer has additional instructions.",
            ),
        }
    }
}
