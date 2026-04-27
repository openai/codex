use codex_core_skills::model::SkillMetadata;
use codex_plugin::PluginCapabilitySummary;

use crate::skills_helpers::skill_description;
use crate::skills_helpers::skill_display_name;

use super::candidate::Candidate;
use super::candidate::MentionType;
use super::candidate::Selection;

pub(crate) fn build_search_catalog(
    skills: Option<&[SkillMetadata]>,
    plugins: Option<&[PluginCapabilitySummary]>,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    if let Some(skills) = skills {
        candidates.extend(skills.iter().map(skill_candidate));
    }

    if let Some(plugins) = plugins {
        candidates.extend(plugins.iter().map(plugin_candidate));
    }

    candidates
}

fn skill_candidate(skill: &SkillMetadata) -> Candidate {
    let display_name = skill_display_name(skill);
    let description = optional_skill_description(skill);
    let skill_name = skill.name.clone();
    let search_terms = if display_name == skill.name {
        vec![skill_name.clone()]
    } else {
        vec![skill_name.clone(), display_name.clone()]
    };
    Candidate {
        display_name,
        description,
        search_terms,
        mention_type: MentionType::Skill,
        selection: Selection::Tool {
            insert_text: format!("${skill_name}"),
            path: Some(skill.path_to_skills_md.to_string_lossy().into_owned()),
        },
    }
}

fn plugin_candidate(plugin: &PluginCapabilitySummary) -> Candidate {
    let (plugin_name, marketplace_name) = plugin
        .config_name
        .split_once('@')
        .unwrap_or((plugin.config_name.as_str(), ""));
    let mut search_terms = vec![plugin_name.to_string(), plugin.config_name.clone()];
    if plugin.display_name != plugin_name {
        search_terms.push(plugin.display_name.clone());
    }
    if !marketplace_name.is_empty() {
        search_terms.push(marketplace_name.to_string());
    }

    Candidate {
        display_name: plugin.display_name.clone(),
        description: plugin_description(plugin),
        search_terms,
        mention_type: MentionType::Plugin,
        selection: Selection::Tool {
            insert_text: format!("${plugin_name}"),
            path: Some(format!("plugin://{}", plugin.config_name)),
        },
    }
}

fn plugin_description(plugin: &PluginCapabilitySummary) -> Option<String> {
    let capability_labels = plugin_capability_labels(plugin);
    plugin.description.clone().or_else(|| {
        Some(if capability_labels.is_empty() {
            "Plugin".to_string()
        } else {
            format!("Plugin - {}", capability_labels.join(" - "))
        })
    })
}

fn plugin_capability_labels(plugin: &PluginCapabilitySummary) -> Vec<String> {
    let mut labels = Vec::new();
    if plugin.has_skills {
        labels.push("skills".to_string());
    }
    if !plugin.mcp_server_names.is_empty() {
        let mcp_server_count = plugin.mcp_server_names.len();
        labels.push(if mcp_server_count == 1 {
            "1 MCP server".to_string()
        } else {
            format!("{mcp_server_count} MCP servers")
        });
    }
    if !plugin.app_connector_ids.is_empty() {
        let app_count = plugin.app_connector_ids.len();
        labels.push(if app_count == 1 {
            "1 app".to_string()
        } else {
            format!("{app_count} apps")
        });
    }
    labels
}

fn optional_skill_description(skill: &SkillMetadata) -> Option<String> {
    let description = skill_description(skill).trim();
    (!description.is_empty()).then(|| description.to_string())
}
