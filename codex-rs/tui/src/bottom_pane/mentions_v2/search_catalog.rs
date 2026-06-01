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
    let mention_name = plugin_mention_name(plugin_name, plugin.display_name.as_str());
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
            insert_text: format!("@{mention_name}"),
            path: Some(format!("plugin://{}", plugin.config_name)),
        },
    }
}

fn plugin_mention_name(plugin_name: &str, display_name: &str) -> String {
    let plugin_segments = split_plugin_name_segments(plugin_name);
    let display_segments = split_display_name_segments(display_name);

    if plugin_segments.len() == display_segments.len()
        && plugin_segments.iter().zip(&display_segments).all(
            |((plugin_segment, _), display_segment)| {
                plugin_segment.eq_ignore_ascii_case(display_segment.as_str())
            },
        )
    {
        let mut result = String::new();
        for ((_, separator), display_segment) in plugin_segments.into_iter().zip(display_segments) {
            result.push_str(display_segment.as_str());
            if let Some(separator) = separator {
                result.push(separator);
            }
        }
        return result;
    }

    title_case_plugin_name(plugin_name)
}

fn split_plugin_name_segments(plugin_name: &str) -> Vec<(String, Option<char>)> {
    let mut segments = Vec::new();
    let mut current = String::new();

    for ch in plugin_name.chars() {
        if matches!(ch, '-' | '_') {
            if !current.is_empty() {
                segments.push((std::mem::take(&mut current), Some(ch)));
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        segments.push((current, None));
    }

    segments
}

fn split_display_name_segments(display_name: &str) -> Vec<String> {
    display_name
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn title_case_plugin_name(plugin_name: &str) -> String {
    let mut result = String::with_capacity(plugin_name.len());
    let mut capitalize_next = true;

    for ch in plugin_name.chars() {
        if matches!(ch, '-' | '_') {
            capitalize_next = true;
            result.push(ch);
            continue;
        }

        if capitalize_next && ch.is_ascii_alphabetic() {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
            capitalize_next = false;
        }
    }

    result
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn plugin_mention_name_uses_display_segments_when_they_match_plugin_name() {
        assert_eq!(
            plugin_mention_name("mcp-search", "MCP Search"),
            "MCP-Search"
        );
        assert_eq!(
            plugin_mention_name("google_calendar", "Google Calendar"),
            "Google_Calendar"
        );
    }

    #[test]
    fn plugin_mention_name_falls_back_to_title_cased_plugin_name() {
        assert_eq!(plugin_mention_name("sample", "Sample Plugin"), "Sample");
        assert_eq!(
            plugin_mention_name("browser-use", "Browser Use"),
            "Browser-Use"
        );
    }
}
