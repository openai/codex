use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AvailablePluginsInstructions;

impl ContextualUserFragment for AvailablePluginsInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            PLUGINS_INSTRUCTIONS_OPEN_TAG,
            PLUGINS_INSTRUCTIONS_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        let mut lines = vec![
            "## Plugins".to_string(),
            "A plugin is a local bundle of skills, MCP servers, and apps.".to_string(),
        ];

        lines.push("### How to use plugins".to_string());
        lines.push(
            r###"- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.
- MCP naming: Plugin-provided MCP tools keep standard MCP identifiers such as `mcp__server__tool`; use tool provenance to tell which plugin they come from.
- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.
- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.
- Relevance: Determine what a plugin can help with from explicit user mention or from the plugin-associated skills, MCP tools, and apps exposed elsewhere in this turn.
- Missing/blocked: If the user requests a plugin that does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback."###
                .to_string(),
        );

        format!("\n{}\n", lines.join("\n"))
    }
}
