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
        plugin_namespace: None,
        description: Some("inspect sample data".to_string()),
        has_skills: true,
        ..PluginCapabilitySummary::default()
    }])
    .expect("plugin section should render");

    let expected = "<plugins_instructions>\n## Plugins\nA plugin is a local bundle of skills, MCP servers, and apps.\n### How to use plugins\n- Skill naming: If a plugin contributes skills, those skill entries are prefixed with `plugin_name:` in the Skills list.\n- MCP naming: Plugin-provided MCP tools keep standard MCP identifiers such as `mcp__server__tool`; use tool provenance to tell which plugin they come from.\n- Trigger rules: If the user explicitly names a plugin, prefer capabilities associated with that plugin for that turn.\n- Relationship to capabilities: Plugins are not invoked directly. Use their underlying skills, MCP tools, and app tools to help solve the task.\n- Relevance: Determine what a plugin can help with from explicit user mention or from the plugin-associated skills, MCP tools, and apps exposed elsewhere in this turn.\n- Missing/blocked: If the user requests a plugin that does not have relevant callable capabilities for the task, say so briefly and continue with the best fallback.\n</plugins_instructions>";

    assert_eq!(rendered, expected);
}

#[test]
fn explicit_plugin_instructions_use_manifest_namespace_for_skills() {
    let rendered = render_explicit_plugin_instructions(
        &PluginCapabilitySummary {
            config_name: "acme.tools@test".to_string(),
            display_name: "Acme Developer Tools".to_string(),
            plugin_namespace: Some("acme.tools".to_string()),
            has_skills: true,
            ..PluginCapabilitySummary::default()
        },
        &[],
        &[],
    )
    .expect("skill capability should render");

    assert!(rendered.contains("`acme.tools:`"));
    assert!(!rendered.contains("`Acme Developer Tools:`"));
    assert!(!rendered.contains("tool_search"));
}

#[test]
fn explicit_plugin_instructions_search_available_apps_before_fallback() {
    let rendered = render_explicit_plugin_instructions(
        &PluginCapabilitySummary {
            config_name: "app-adobe@openai-curated-remote".to_string(),
            display_name: "Adobe".to_string(),
            ..PluginCapabilitySummary::default()
        },
        &[],
        &["Adobe".to_string()],
    )
    .expect("app capability should render");

    assert_eq!(
        rendered,
        "Capabilities from the `Adobe` plugin:\n\
         - For the user request that explicitly selected this plugin, and only for that request, \
         if `tool_search` is available and an app from this plugin may help, search for its \
         tools before falling back to unrelated or built-in tools.\n\
         - Apps from this plugin available in this session: `Adobe`.\n\
         Use these plugin-associated capabilities to help solve the task."
    );
}

#[test]
fn explicit_plugin_instructions_are_bounded() {
    let servers = (0..1_024)
        .map(|index| format!("server-{index}"))
        .collect::<Vec<_>>();
    let apps = (0..1_024)
        .map(|index| format!("app-{index}"))
        .collect::<Vec<_>>();

    let rendered = render_explicit_plugin_instructions(
        &PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            has_skills: true,
            ..PluginCapabilitySummary::default()
        },
        &servers,
        &apps,
    )
    .expect("MCP capability should render");

    assert!(rendered.len() <= MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES);
    assert!(rendered.contains("only for that request"));
    assert!(rendered.contains("if `tool_search` is available"));
    assert!(rendered.contains("Skills from this plugin"));
    assert!(rendered.contains("`app-0`"));
    assert!(rendered.ends_with(TRUNCATED_PLUGIN_INSTRUCTIONS_SUFFIX));
}
