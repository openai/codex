use std::collections::HashSet;

use codex_protocol::user_input::UserInput;

use crate::injection::ToolMentionKind;
use crate::injection::app_id_from_path;
use crate::injection::extract_tool_mentions_with_sigil;
use crate::injection::plugin_config_name_from_path;
use crate::injection::tool_kind_for_path;
use crate::mention_syntax::PLUGIN_TEXT_MENTION_SIGIL;
use crate::mention_syntax::TOOL_MENTION_SIGIL;

use super::PluginCapabilitySummary;

fn collect_tool_paths_from_messages(messages: &[String]) -> HashSet<String> {
    collect_tool_paths_from_messages_with_sigil(messages, TOOL_MENTION_SIGIL)
}

fn collect_tool_paths_from_messages_with_sigil(
    messages: &[String],
    sigil: char,
) -> HashSet<String> {
    let mut paths = HashSet::new();
    for message in messages {
        let mentions = extract_tool_mentions_with_sigil(message, sigil);
        paths.extend(mentions.paths().map(str::to_string));
    }
    paths
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
        .chain(collect_tool_paths_from_messages(&messages))
        .filter(|path| tool_kind_for_path(path.as_str()) == ToolMentionKind::App)
        .filter_map(|path| app_id_from_path(path.as_str()).map(str::to_string))
        .collect()
}

/// Collect explicit structured or linked `plugin://...` mentions.
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
        .chain(
            // Plugin plaintext links use `@`, not the default `$` tool sigil.
            collect_tool_paths_from_messages_with_sigil(&messages, PLUGIN_TEXT_MENTION_SIGIL),
        )
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

#[cfg(test)]
#[path = "mentions_tests.rs"]
mod tests;
