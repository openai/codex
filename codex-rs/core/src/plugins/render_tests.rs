use super::*;
use pretty_assertions::assert_eq;

#[test]
fn render_plugin_instructions_returns_none_for_empty_plugins() {
    assert_eq!(render_plugin_instructions(&[]), None);
}

#[test]
fn render_plugins_section_includes_descriptions_and_skill_naming_guidance() {
    let rendered = render_plugin_instructions(&[PluginCapabilitySummary {
        config_name: "sample@test".to_string(),
        display_name: "sample".to_string(),
        description: Some("inspect sample data".to_string()),
        has_skills: true,
        ..PluginCapabilitySummary::default()
    }])
    .unwrap_or_else(|| panic!("plugin section should render"));

    let expected = "<plugins_instructions>\n## Plugins\nA plugin is a local bundle of skills, MCP servers, and apps. Below is the list of plugins that are enabled and available in this session.\n### Available plugins\n- `sample`: inspect sample data\n### How to use plugins\n- Discovery: The list above is the plugins available in this session.\n- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.\n- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.\n- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.\n- Preference: When a relevant plugin is available, prefer using capabilities associated with that plugin over standalone capabilities that provide similar functionality.\n- Missing/blocked: If the user requests a plugin that is not listed above, or the plugin does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback.\n</plugins_instructions>";

    assert_eq!(rendered.text, expected);
}
