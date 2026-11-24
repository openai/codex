//! Resolves incremental catalog wording without changing catalog identity or update timing.
//! Text is owned because world-state sections outlive the borrowed model metadata.

use codex_protocol::openai_models::IncrementalToolMessages;
use codex_utils_cached_str::CodexCachedStr;

/// Resolved static wording; consumers retain ownership of runtime names and instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIncrementalToolMessages {
    pub tool_update_hint: CodexCachedStr,
    pub removed_tools_header: CodexCachedStr,
    pub removed_namespaces_header: CodexCachedStr,
    pub namespace_instructions_prefix: CodexCachedStr,
    pub namespace_instructions_cleared: CodexCachedStr,
    pub exec_tools_update_hint: CodexCachedStr,
    pub exec_tools_removed_definitions_header: CodexCachedStr,
}

impl Default for ResolvedIncrementalToolMessages {
    fn default() -> Self {
        Self::new(/*messages*/ None)
    }
}

impl ResolvedIncrementalToolMessages {
    pub(crate) fn new(messages: Option<&IncrementalToolMessages>) -> Self {
        let resolve = |text: Option<&CodexCachedStr>, bundled: &str| {
            text.filter(|text| text.len() <= 512)
                .map_or_else(|| bundled.into(), Clone::clone)
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
            exec_tools_update_hint: resolve(
                messages.and_then(|m| m.exec_tools_update_hint.as_ref()),
                "Use these definitions and namespace instructions with `functions.exec`. Each replaces its earlier version in full. Unlisted tools keep their latest definitions and `tools` methods.",
            ),
            exec_tools_removed_definitions_header: resolve(
                messages.and_then(|m| m.exec_tools_removed_definitions_header.as_ref()),
                "Listed `Tool:` entries are unavailable until redefined or rediscovered. `Namespace instructions:` removes guidance only. Unlisted entries are unchanged.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolving a catalog override shares its storage, including when it matches a default.
    #[test]
    fn resolved_overrides_share_storage_with_catalog_and_defaults() {
        let defaults = ResolvedIncrementalToolMessages::default();
        let catalog = IncrementalToolMessages {
            tool_update_hint: Some(defaults.tool_update_hint.as_str().into()),
            removed_tools_header: Some("Custom removal instructions".into()),
            ..Default::default()
        };
        let resolved = ResolvedIncrementalToolMessages::new(Some(&catalog));
        assert!(std::ptr::eq(
            defaults.tool_update_hint.as_str(),
            resolved.tool_update_hint.as_str()
        ));
        assert!(std::ptr::eq(
            catalog.removed_tools_header.as_deref().unwrap(),
            resolved.removed_tools_header.as_str()
        ));
    }
}
