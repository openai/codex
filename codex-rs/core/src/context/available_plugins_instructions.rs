use codex_plugin::PluginCapabilitySummary;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AvailablePluginsInstructions {
    plugins: Vec<PluginCapabilitySummary>,
}

impl AvailablePluginsInstructions {
    pub(crate) fn from_plugins(plugins: &[PluginCapabilitySummary]) -> Option<Self> {
        if plugins.is_empty() {
            return None;
        }

        Some(Self {
            plugins: plugins.to_vec(),
        })
    }
}

impl ContextualUserFragment for AvailablePluginsInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = PLUGINS_INSTRUCTIONS_OPEN_TAG;
    const END_MARKER: &'static str = PLUGINS_INSTRUCTIONS_CLOSE_TAG;

    fn body(&self) -> String {
        let mut lines = vec![
            "## Plugins".to_string(),
            "A plugin is a local bundle of skills, MCP servers, and apps. Below is the list of plugins that are enabled and available in this session.".to_string(),
            "### Available plugins".to_string(),
        ];

        lines.extend(
            self.plugins
                .iter()
                .map(|plugin| match plugin.description.as_deref() {
                    Some(description) => format!("- `{}`: {description}", plugin.display_name),
                    None => format!("- `{}`", plugin.display_name),
                }),
        );

        lines.push("### How to use plugins".to_string());
        lines.push(
            r###"- Discovery: The list above is the plugins available in this session.
- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.
- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.
- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.
- Preference: When a relevant plugin is available, prefer using capabilities associated with that plugin over standalone capabilities that provide similar functionality.
- Missing/blocked: If the user requests a plugin that is not listed above, or the plugin does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback."###
                .to_string(),
        );

        format!("\n{}\n", lines.join("\n"))
    }
}
