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
        &Features::default(),
        &codex_prompts::ResolvedModelMessages::bundled().selected_plugin(),
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
        &Features::default(),
        &codex_prompts::ResolvedModelMessages::bundled().selected_plugin(),
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
        &Features::default(),
        &codex_prompts::ResolvedModelMessages::bundled().selected_plugin(),
    )
    .expect("MCP capability should render");

    assert!(rendered.len() <= MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES);
    assert!(rendered.contains("only for that request"));
    assert!(rendered.contains("if `tool_search` is available"));
    assert!(rendered.contains("Skills from this plugin"));
    assert!(rendered.contains("`app-0`"));
    assert!(
        rendered.ends_with(
            &codex_prompts::ResolvedModelMessages::bundled()
                .selected_plugin()
                .omitted
        )
    );
}

#[test]
fn selected_plugin_templates_insert_runtime_facts_once() {
    let mut messages = codex_prompts::ResolvedModelMessages::bundled().selected_plugin();
    messages.intro = "Selected {{ plugin_name }}.".into();
    messages.search_tools = "Search wording {{literal}}.\r\n".into();
    messages.skill_namespace = "Skills {{ skill_namespace }}.".into();
    messages.apps = "Apps {{ apps }}.".into();
    messages.mcp_servers = "Servers {{ mcp_servers }}.".into();
    messages.closing = "Closing {{literal}}.".into();
    let rendered = render_explicit_plugin_instructions(
        &PluginCapabilitySummary {
            display_name: "Name {{ apps }}".into(),
            plugin_namespace: Some("namespace{{ plugin_name }}".into()),
            has_skills: true,
            ..PluginCapabilitySummary::default()
        },
        &["server{{ apps }}".into()],
        &["app{{ mcp_servers }}".into()],
        &Features::default(),
        &messages,
    )
    .unwrap();
    assert_eq!(
        rendered,
        "Selected Name {{ apps }}.\nSearch wording {{literal}}.\r\n\nSkills namespace{{ plugin_name }}.\nApps `app{{ mcp_servers }}`.\nServers `server{{ apps }}`.\nClosing {{literal}}."
    );
}

#[test]
fn selected_plugin_omission_keeps_utf8_and_the_final_item_bound() {
    let mut messages = codex_prompts::ResolvedModelMessages::bundled().selected_plugin();
    messages.omitted = "é".repeat(250);
    let rendered = render_explicit_plugin_instructions(
        &PluginCapabilitySummary {
            display_name: "plugin".into(),
            ..Default::default()
        },
        &[],
        &["🦀".repeat(2_000)],
        &Features::default(),
        &messages,
    )
    .unwrap();
    assert!(rendered.len() <= MAX_EXPLICIT_PLUGIN_INSTRUCTIONS_BYTES);
    assert!(rendered.ends_with(&format!("\n{}", messages.omitted)));
    assert!(rendered.contains("🦀"));
}
