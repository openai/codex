use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use codex_protocol::user_input::UserInput;

use crate::connectors;
use crate::plugins::PluginCapabilitySummary;
use crate::skills::SkillMetadata;
use crate::skills::injection::ToolMentionKind;
use crate::skills::injection::app_id_from_path;
use crate::skills::injection::extract_tool_mentions;
use crate::skills::injection::plugin_config_name_from_path;
use crate::skills::injection::tool_kind_for_path;

pub(crate) struct CollectedToolMentions {
    pub(crate) plain_names: HashSet<String>,
    pub(crate) paths: HashSet<String>,
}

pub(crate) fn collect_tool_mentions_from_messages(messages: &[String]) -> CollectedToolMentions {
    let mut plain_names = HashSet::new();
    let mut paths = HashSet::new();
    for message in messages {
        let mentions = extract_tool_mentions(message);
        plain_names.extend(mentions.plain_names().map(str::to_string));
        paths.extend(mentions.paths().map(str::to_string));
    }
    CollectedToolMentions { plain_names, paths }
}

pub(crate) fn collect_explicit_app_ids(input: &[UserInput]) -> HashSet<String> {
    let messages = input
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<String>>();

    input
        .iter()
        .filter_map(|item| match item {
            UserInput::Mention { path, .. } => Some(path.clone()),
            _ => None,
        })
        .chain(collect_tool_mentions_from_messages(&messages).paths)
        .filter(|path| tool_kind_for_path(path.as_str()) == ToolMentionKind::App)
        .filter_map(|path| app_id_from_path(path.as_str()).map(str::to_string))
        .collect()
}

/// Collect explicit structured `plugin://...` mentions.
pub(crate) fn collect_explicit_plugin_mentions(
    input: &[UserInput],
    plugins: &[PluginCapabilitySummary],
) -> Vec<PluginCapabilitySummary> {
    if plugins.is_empty() {
        return Vec::new();
    }

    let messages = input
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<String>>();

    let mentioned_config_names: HashSet<String> = input
        .iter()
        .filter_map(|item| match item {
            UserInput::Mention { path, .. } => Some(path.clone()),
            _ => None,
        })
        .chain(collect_tool_mentions_from_messages(&messages).paths)
        .filter(|path| tool_kind_for_path(path.as_str()) == ToolMentionKind::Plugin)
        .filter_map(|path| plugin_config_name_from_path(path.as_str()).map(str::to_string))
        .collect();

    if mentioned_config_names.is_empty() {
        return Vec::new();
    }

    plugins
        .iter()
        .filter(|plugin| mentioned_config_names.contains(plugin.config_name.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn build_skill_name_counts(
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut exact_counts: HashMap<String, usize> = HashMap::new();
    let mut lower_counts: HashMap<String, usize> = HashMap::new();
    for skill in skills {
        if disabled_paths.contains(&skill.path_to_skills_md) {
            continue;
        }
        *exact_counts.entry(skill.name.clone()).or_insert(0) += 1;
        *lower_counts
            .entry(skill.name.to_ascii_lowercase())
            .or_insert(0) += 1;
    }
    (exact_counts, lower_counts)
}

pub(crate) fn build_connector_slug_counts(
    connectors: &[connectors::AppInfo],
) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for connector in connectors {
        let slug = connectors::connector_mention_slug(connector);
        *counts.entry(slug).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use codex_protocol::user_input::UserInput;
    use pretty_assertions::assert_eq;

    use super::collect_explicit_app_ids;
    use super::collect_explicit_plugin_mentions;
    use crate::plugins::PluginCapabilitySummary;

    fn text_input(text: &str) -> UserInput {
        UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }
    }

    fn plugin(config_name: &str, display_name: &str) -> PluginCapabilitySummary {
        PluginCapabilitySummary {
            config_name: config_name.to_string(),
            display_name: display_name.to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        }
    }

    #[test]
    fn collect_explicit_app_ids_from_linked_text_mentions() {
        let input = vec![text_input("use [$calendar](app://calendar)")];

        let app_ids = collect_explicit_app_ids(&input);

        assert_eq!(app_ids, HashSet::from(["calendar".to_string()]));
    }

    #[test]
    fn collect_explicit_app_ids_dedupes_structured_and_linked_mentions() {
        let input = vec![
            text_input("use [$calendar](app://calendar)"),
            UserInput::Mention {
                name: "calendar".to_string(),
                path: "app://calendar".to_string(),
            },
        ];

        let app_ids = collect_explicit_app_ids(&input);

        assert_eq!(app_ids, HashSet::from(["calendar".to_string()]));
    }

    #[test]
    fn collect_explicit_app_ids_ignores_non_app_paths() {
        let input = vec![
            text_input(
                "use [$docs](mcp://docs) and [$skill](skill://team/skill) and [$file](/tmp/file.txt)",
            ),
            UserInput::Mention {
                name: "docs".to_string(),
                path: "mcp://docs".to_string(),
            },
            UserInput::Mention {
                name: "skill".to_string(),
                path: "skill://team/skill".to_string(),
            },
            UserInput::Mention {
                name: "file".to_string(),
                path: "/tmp/file.txt".to_string(),
            },
        ];

        let app_ids = collect_explicit_app_ids(&input);

        assert_eq!(app_ids, HashSet::<String>::new());
    }

    #[test]
    fn collect_explicit_plugin_mentions_from_structured_paths() {
        let plugins = vec![
            plugin("sample@test", "sample"),
            plugin("other@test", "other"),
        ];

        let mentioned = collect_explicit_plugin_mentions(
            &[UserInput::Mention {
                name: "sample".to_string(),
                path: "plugin://sample@test".to_string(),
            }],
            &plugins,
        );

        assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_from_linked_text_mentions() {
        let plugins = vec![
            plugin("sample@test", "sample"),
            plugin("other@test", "other"),
        ];

        let mentioned = collect_explicit_plugin_mentions(
            &[text_input("use [$sample](plugin://sample@test)")],
            &plugins,
        );

        assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_dedupes_structured_and_linked_mentions() {
        let plugins = vec![
            plugin("sample@test", "sample"),
            plugin("other@test", "other"),
        ];

        let mentioned = collect_explicit_plugin_mentions(
            &[
                text_input("use [$sample](plugin://sample@test)"),
                UserInput::Mention {
                    name: "sample".to_string(),
                    path: "plugin://sample@test".to_string(),
                },
            ],
            &plugins,
        );

        assert_eq!(mentioned, vec![plugin("sample@test", "sample")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_ignores_non_plugin_paths() {
        let plugins = vec![plugin("sample@test", "sample")];

        let mentioned = collect_explicit_plugin_mentions(
            &[text_input(
                "use [$app](app://calendar) and [$skill](skill://team/skill) and [$file](/tmp/file.txt)",
            )],
            &plugins,
        );

        assert_eq!(mentioned, Vec::<PluginCapabilitySummary>::new());
    }
}
