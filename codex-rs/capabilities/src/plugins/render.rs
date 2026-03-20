use crate::plugins::PluginCapabilitySummary;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;

pub fn render_plugins_section(plugins: &[PluginCapabilitySummary]) -> Option<String> {
    if plugins.is_empty() {
        return None;
    }

    let mut lines = vec![
        "## Plugins".to_string(),
        "A plugin is a local bundle of skills, MCP servers, and apps. Below is the list of plugins that are enabled and available in this session.".to_string(),
        "### Available plugins".to_string(),
    ];

    lines.extend(
        plugins
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

    let body = lines.join("\n");
    Some(format!(
        "{PLUGINS_INSTRUCTIONS_OPEN_TAG}\n{body}\n{PLUGINS_INSTRUCTIONS_CLOSE_TAG}"
    ))
}

pub fn render_explicit_plugin_instructions(
    plugin: &PluginCapabilitySummary,
    available_mcp_servers: &[String],
    available_apps: &[String],
) -> Option<String> {
    let mut lines = vec![format!(
        "Capabilities from the `{}` plugin:",
        plugin.display_name
    )];

    if plugin.has_skills {
        lines.push(format!(
            "- Skills from this plugin are prefixed with `{}:`.",
            plugin.display_name
        ));
    }

    if !available_mcp_servers.is_empty() {
        lines.push(format!(
            "- MCP servers from this plugin available in this session: {}.",
            available_mcp_servers
                .iter()
                .map(|server| format!("`{server}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !available_apps.is_empty() {
        lines.push(format!(
            "- Apps from this plugin available in this session: {}.",
            available_apps
                .iter()
                .map(|app| format!("`{app}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if lines.len() == 1 {
        return None;
    }

    lines.push("Use these plugin-associated capabilities to help solve the task.".to_string());

    Some(lines.join("\n"))
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
