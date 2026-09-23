//! Extracts confirmed User Messaging text after input-rewriting hooks.
//! The per-call runtime state retains this evidence independently of post-tool hooks.

use crate::guardian::GUARDIAN_MAX_ROOT_MESSAGE_TOKENS;
use crate::guardian::guardian_truncate_text;
use crate::tools::registry::AnyToolResult;

impl AnyToolResult {
    pub(crate) fn delivered_assistant_message(&self) -> Option<String> {
        if !self.result.success_for_logging() {
            return None;
        }
        let payload = self.post_tool_use_payload.as_ref()?;
        // MCP hook names normalize prefixed/unprefixed and flat/namespaced calls.
        // Both connector spellings also have a form for catalogs without connector metadata.
        if !matches!(
            payload.tool_name.name(),
            "mcp__codex_apps__user_messaging__send_message"
                | "mcp__codex_apps__user_messaging_send_message"
                | "mcp__codex_apps__user_message__send_message"
                | "mcp__codex_apps__user_message_send_message"
        ) {
            return None;
        }
        let text = payload.tool_input.get("text")?.as_str()?;
        if text.trim().is_empty() {
            return None;
        }
        Some(guardian_truncate_text(text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS).0)
    }
}

#[cfg(test)]
#[path = "user_messaging_tests.rs"]
mod tests;
