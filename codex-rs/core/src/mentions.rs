use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;

use crate::compact::collect_user_messages;
use crate::connectors;
use crate::instructions::SkillInstructions;
use crate::skills::SkillMetadata;
use crate::skills::injection::ToolMentionKind;
use crate::skills::injection::extract_tool_mentions;
use crate::skills::injection::tool_kind_for_path;

#[derive(Debug, Clone, Default)]
pub(crate) struct CollectedToolMentions {
    pub(crate) plain_names: HashSet<String>,
    pub(crate) paths: HashSet<String>,
}

impl CollectedToolMentions {
    pub(crate) fn is_empty(&self) -> bool {
        self.plain_names.is_empty() && self.paths.is_empty()
    }

    pub(crate) fn extend_from(&mut self, other: &CollectedToolMentions) {
        self.plain_names.extend(other.plain_names.iter().cloned());
        self.paths.extend(other.paths.iter().cloned());
    }
}

pub(crate) fn collect_tool_mentions_from_messages(messages: &[String]) -> CollectedToolMentions {
    collect_tool_mentions_from_texts(messages.iter().map(String::as_str))
}

pub(crate) fn collect_tool_mentions_from_texts<'a, I>(messages: I) -> CollectedToolMentions
where
    I: IntoIterator<Item = &'a str>,
{
    let mut plain_names = HashSet::new();
    let mut paths = HashSet::new();
    for message in messages {
        let mentions = extract_tool_mentions(message);
        plain_names.extend(mentions.plain_names().map(str::to_string));
        paths.extend(mentions.paths().map(str::to_string));
    }
    CollectedToolMentions { plain_names, paths }
}

pub(crate) fn collect_structured_tool_mentions_from_user_input(
    input: &[UserInput],
) -> CollectedToolMentions {
    let mut mentions = CollectedToolMentions::default();
    for item in input {
        match item {
            UserInput::Mention { name, path } => {
                mentions.plain_names.insert(name.clone());
                mentions.paths.insert(path.clone());
            }
            UserInput::Skill { name, path } => {
                mentions.plain_names.insert(name.clone());
                mentions.paths.insert(path.to_string_lossy().into_owned());
            }
            UserInput::Text { .. } | UserInput::Image { .. } | UserInput::LocalImage { .. } => {}
            _ => {}
        }
    }

    mentions
}

pub(crate) fn collect_tool_mentions_from_response_items(
    items: &[ResponseItem],
) -> CollectedToolMentions {
    let messages = collect_user_messages(items);
    collect_tool_mentions_from_messages(&messages)
}

pub(crate) fn collect_skill_instruction_paths(items: &[ResponseItem]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for item in items {
        let ResponseItem::Message { role, content, .. } = item else {
            continue;
        };
        if role != "user" {
            continue;
        }
        let Some(path) = SkillInstructions::extract_path(content) else {
            continue;
        };
        paths.insert(path);
    }
    paths
}

pub(crate) fn collect_app_ids_from_mentions(mentions: &CollectedToolMentions) -> HashSet<String> {
    mentions
        .paths
        .iter()
        .filter(|path| tool_kind_for_path(path) == ToolMentionKind::App)
        .filter_map(|path| crate::skills::injection::app_id_from_path(path))
        .map(str::to_string)
        .collect()
}

pub(crate) fn build_skill_name_counts(
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut exact_counts: HashMap<String, usize> = HashMap::new();
    let mut lower_counts: HashMap<String, usize> = HashMap::new();
    for skill in skills {
        if disabled_paths.contains(&skill.path) {
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
