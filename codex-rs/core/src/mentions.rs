use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use codex_protocol::models::ResponseItem;

use crate::connectors;
use crate::skills::SkillMetadata;
use crate::skills::injection::extract_tool_mentions;

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

/// Extracts all mentioned resource paths from tool-output text so connector and skill
/// references emitted by tools can affect follow-up tool selection.
pub(crate) fn collect_tool_paths_from_tool_outputs(input: &[ResponseItem]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for item in input {
        let text = match item {
            ResponseItem::FunctionCallOutput { output, .. } => output.body.to_text(),
            ResponseItem::CustomToolCallOutput { output, .. } => Some(output.clone()),
            _ => None,
        };
        if let Some(text) = text {
            let mentions = extract_tool_mentions(&text);
            paths.extend(mentions.paths().map(str::to_string));
        }
    }
    paths
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
