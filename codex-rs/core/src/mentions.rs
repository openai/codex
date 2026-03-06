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

/// Collect explicit plain-text `@plugin` mentions from user text.
///
/// This is currently the core-side fallback path for plugin mentions. It
/// matches unambiguous plugin `display_name`s from the filtered capability
/// index, case-insensitively, by scanning for exact `@display name` matches.
///
/// It is hand-rolled because core only has a `$...` / `[$...](...)` mention
/// parser today, and the existing TUI `@...` logic is file-autocomplete, not
/// turn-time parsing.
///
/// Long term, explicit plugin picks should come through structured
/// `plugin://...` mentions, likely via `UserInput::Mention`, once clients can list
/// plugins and the UI has plugin-mention support (likely a plugins/list app-server
/// endpoint). Even then, this may stay as a text fallback, similar to skills/apps.
pub(crate) fn collect_explicit_plugin_mentions(
    input: &[UserInput],
    plugins: &[PluginCapabilitySummary],
) -> Vec<PluginCapabilitySummary> {
    if plugins.is_empty() {
        return Vec::new();
    }

    let mut display_name_counts = HashMap::new();
    for plugin in plugins {
        *display_name_counts
            .entry(plugin.display_name.to_lowercase())
            .or_insert(0) += 1;
    }

    let mut display_names = display_name_counts.keys().cloned().collect::<Vec<_>>();
    display_names.sort_by_key(|display_name| std::cmp::Reverse(display_name.len()));

    let mut mentioned_display_names = HashSet::new();
    for text in input.iter().filter_map(|item| match item {
        UserInput::Text { text, .. } => Some(text.as_str()),
        _ => None,
    }) {
        let text = text.to_lowercase();
        let mut index = 0;
        while let Some(relative_at_sign) = text[index..].find('@') {
            let at_sign = index + relative_at_sign;
            if text[..at_sign]
                .chars()
                .next_back()
                .is_some_and(is_plugin_mention_body_char)
            {
                index = at_sign + 1;
                continue;
            }

            let Some((matched_display_name, matched_len)) =
                display_names.iter().find_map(|display_name| {
                    text[at_sign + 1..].starts_with(display_name).then(|| {
                        let end = at_sign + 1 + display_name.len();
                        text[end..]
                            .chars()
                            .next()
                            .is_none_or(|ch| !is_plugin_mention_body_char(ch))
                            .then_some((display_name, display_name.len()))
                    })?
                })
            else {
                index = at_sign + 1;
                continue;
            };

            if display_name_counts
                .get(matched_display_name)
                .copied()
                .unwrap_or(0)
                == 1
            {
                mentioned_display_names.insert(matched_display_name.clone());
            }
            index = at_sign + 1 + matched_len;
        }
    }

    if mentioned_display_names.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut seen_display_names = HashSet::new();
    for plugin in plugins {
        let display_name = plugin.display_name.to_lowercase();
        if !mentioned_display_names.contains(&display_name) {
            continue;
        }
        if seen_display_names.insert(display_name) {
            selected.push(plugin.clone());
        }
    }

    selected
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

fn is_plugin_mention_body_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | ':')
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

    fn plugin(display_name: &str) -> PluginCapabilitySummary {
        PluginCapabilitySummary {
            config_name: format!("{display_name}@test"),
            display_name: display_name.to_string(),
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
    fn collect_explicit_plugin_mentions_resolves_unique_display_names() {
        let plugins = vec![plugin("sample"), plugin("other")];

        let mentioned = collect_explicit_plugin_mentions(&[text_input("use @sample")], &plugins);

        assert_eq!(mentioned, vec![plugin("sample")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_resolves_non_slug_display_names() {
        let spaced_plugins = vec![plugin("Google Calendar")];
        let spaced_mentioned = collect_explicit_plugin_mentions(
            &[text_input("use @Google Calendar")],
            &spaced_plugins,
        );
        assert_eq!(spaced_mentioned, vec![plugin("Google Calendar")]);

        let unicode_plugins = vec![plugin("Café")];
        let unicode_mentioned =
            collect_explicit_plugin_mentions(&[text_input("use @Café")], &unicode_plugins);
        assert_eq!(unicode_mentioned, vec![plugin("Café")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_prefers_longer_display_names() {
        let plugins = vec![plugin("Google"), plugin("Google Calendar")];

        let mentioned =
            collect_explicit_plugin_mentions(&[text_input("use @Google Calendar")], &plugins);

        assert_eq!(mentioned, vec![plugin("Google Calendar")]);
    }

    #[test]
    fn collect_explicit_plugin_mentions_does_not_fall_back_from_ambiguous_longer_name() {
        let plugins = vec![
            plugin("Google"),
            PluginCapabilitySummary {
                config_name: "calendar-1@test".to_string(),
                ..plugin("Google Calendar")
            },
            PluginCapabilitySummary {
                config_name: "calendar-2@test".to_string(),
                ..plugin("Google Calendar")
            },
        ];

        let mentioned =
            collect_explicit_plugin_mentions(&[text_input("use @Google Calendar")], &plugins);

        assert_eq!(mentioned, Vec::<PluginCapabilitySummary>::new());
    }

    #[test]
    fn collect_explicit_plugin_mentions_ignores_embedded_at_signs() {
        let plugins = vec![plugin("sample")];

        let mentioned = collect_explicit_plugin_mentions(
            &[text_input("contact sample@openai.com, do not use plugins")],
            &plugins,
        );

        assert_eq!(mentioned, Vec::<PluginCapabilitySummary>::new());
    }
}
