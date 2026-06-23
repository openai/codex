use super::*;
use pretty_assertions::assert_eq;

#[test]
fn render_plugins_section_returns_none_for_empty_plugins() {
    assert_eq!(render_plugins_section(&[]), None);
}

#[test]
fn render_plugins_section_keeps_plugin_usage_guidance_without_listing_plugins() {
    let rendered = render_plugins_section(&[PluginCapabilitySummary {
        config_name: "sample@test".to_string(),
        display_name: "sample".to_string(),
        description: Some("inspect sample data".to_string()),
        has_skills: true,
        ..PluginCapabilitySummary::default()
    }])
    .expect("plugin section should render");

    let expected = "<plugins_instructions>\n## Plugins\nA plugin is a local bundle of skills, MCP servers, and apps.\n### How to use plugins\n- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.\n- MCP naming: Plugin-provided MCP tools keep standard MCP identifiers such as `mcp__server__tool`; use tool provenance to tell which plugin they come from.\n- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.\n- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.\n- Relevance: Determine what a plugin can help with from explicit user mention or from the plugin-associated skills, MCP tools, and apps exposed elsewhere in this turn.\n- Missing/blocked: If the user requests a plugin that does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback.\n</plugins_instructions>";

    assert_eq!(rendered, expected);
}
