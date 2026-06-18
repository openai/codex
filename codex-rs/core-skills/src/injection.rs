use std::collections::HashMap;
use std::collections::HashSet;

use crate::SkillMetadata;
use crate::build_skill_name_counts;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
pub use codex_utils_plugins::tool_mentions::ToolMentionKind;
pub use codex_utils_plugins::tool_mentions::ToolMentions;
pub use codex_utils_plugins::tool_mentions::app_id_from_path;
pub use codex_utils_plugins::tool_mentions::extract_tool_mentions;
pub use codex_utils_plugins::tool_mentions::extract_tool_mentions_with_sigil;
pub use codex_utils_plugins::tool_mentions::plugin_config_name_from_path;
pub use codex_utils_plugins::tool_mentions::tool_kind_for_path;

/// Collect explicitly mentioned skills from structured and text mentions.
///
/// Structured `UserInput::Skill` selections are resolved first by path against
/// enabled skills. Text inputs are then scanned to extract `$skill-name` tokens, and we
/// iterate `skills` in their existing order to preserve prior ordering semantics.
/// Explicit links are resolved by path and plain names are only used when the match
/// is unambiguous.
///
/// Complexity: `O(T + (N_s + N_t) * S)` time, `O(S + M)` space, where:
/// `S` = number of skills, `T` = total text length, `N_s` = number of structured skill inputs,
/// `N_t` = number of text inputs, `M` = max mentions parsed from a single text input.
pub fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    let skill_name_counts = build_skill_name_counts(skills, disabled_paths).0;

    let selection_context = SkillSelectionContext {
        skills,
        disabled_paths,
        skill_name_counts: &skill_name_counts,
        connector_slug_counts,
    };
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_paths: HashSet<AbsolutePathBuf> = HashSet::new();
    let mut blocked_plain_names: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path, .. } = input {
            blocked_plain_names.insert(name.clone());
            let Ok(path) = AbsolutePathBuf::relative_to_current_dir(path) else {
                continue;
            };

            if selection_context.disabled_paths.contains(&path) || seen_paths.contains(&path) {
                continue;
            }

            if let Some(skill) = selection_context
                .skills
                .iter()
                .find(|skill| skill.path_to_skills_md == path)
            {
                seen_paths.insert(skill.path_to_skills_md.clone());
                seen_names.insert(skill.name.clone());
                selected.push(skill.clone());
            }
        }
    }

    for input in inputs {
        if let UserInput::Text { text, .. } = input {
            let mentioned_names = extract_tool_mentions(text);
            select_skills_from_mentions(
                &selection_context,
                &blocked_plain_names,
                &mentioned_names,
                &mut seen_names,
                &mut seen_paths,
                &mut selected,
            );
        }
    }

    selected
}

struct SkillSelectionContext<'a> {
    skills: &'a [SkillMetadata],
    disabled_paths: &'a HashSet<AbsolutePathBuf>,
    skill_name_counts: &'a HashMap<String, usize>,
    connector_slug_counts: &'a HashMap<String, usize>,
}

const SKILL_PATH_PREFIX: &str = "skill://";

pub(crate) fn normalize_skill_path(path: &str) -> &str {
    path.strip_prefix(SKILL_PATH_PREFIX).unwrap_or(path)
}

/// Select mentioned skills while preserving the order of `skills`.
fn select_skills_from_mentions(
    selection_context: &SkillSelectionContext<'_>,
    blocked_plain_names: &HashSet<String>,
    mentions: &ToolMentions<'_>,
    seen_names: &mut HashSet<String>,
    seen_paths: &mut HashSet<AbsolutePathBuf>,
    selected: &mut Vec<SkillMetadata>,
) {
    if mentions.is_empty() {
        return;
    }

    let mention_skill_paths: HashSet<&str> = mentions
        .paths()
        .filter(|path| {
            !matches!(
                tool_kind_for_path(path),
                ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
            )
        })
        .map(normalize_skill_path)
        .collect();

    for skill in selection_context.skills {
        if selection_context
            .disabled_paths
            .contains(&skill.path_to_skills_md)
            || seen_paths.contains(&skill.path_to_skills_md)
        {
            continue;
        }

        let path_str = skill.path_to_skills_md.to_string_lossy();
        if mention_skill_paths.contains(path_str.as_ref()) {
            seen_paths.insert(skill.path_to_skills_md.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
        }
    }

    for skill in selection_context.skills {
        if selection_context
            .disabled_paths
            .contains(&skill.path_to_skills_md)
            || seen_paths.contains(&skill.path_to_skills_md)
        {
            continue;
        }

        if blocked_plain_names.contains(skill.name.as_str()) {
            continue;
        }
        if !mentions.contains_plain_name(skill.name.as_str()) {
            continue;
        }

        let skill_count = selection_context
            .skill_name_counts
            .get(skill.name.as_str())
            .copied()
            .unwrap_or(0);
        let connector_count = selection_context
            .connector_slug_counts
            .get(&skill.name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0);
        if skill_count != 1 || connector_count != 0 {
            continue;
        }

        if seen_names.insert(skill.name.clone()) {
            seen_paths.insert(skill.path_to_skills_md.clone());
            selected.push(skill.clone());
        }
    }
}

#[cfg(test)]
fn text_mentions_skill(text: &str, skill_name: &str) -> bool {
    if skill_name.is_empty() {
        return false;
    }

    let text_bytes = text.as_bytes();
    let skill_bytes = skill_name.as_bytes();

    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte != b'$' {
            continue;
        }

        let name_start = index + 1;
        let Some(rest) = text_bytes.get(name_start..) else {
            continue;
        };
        if !rest.starts_with(skill_bytes) {
            continue;
        }

        let after_index = name_start + skill_bytes.len();
        let after = text_bytes.get(after_index).copied();
        if after.is_none_or(|b| !is_mention_name_char(b)) {
            return true;
        }
    }

    false
}

#[cfg(test)]
fn is_mention_name_char(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b':')
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
