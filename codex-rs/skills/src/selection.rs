use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::SkillMetadata;
use crate::ToolMentionKind;
use crate::ToolMentions;
use crate::build_skill_name_counts;
use crate::extract_tool_mentions;
use crate::normalize_skill_path;
use crate::tool_kind_for_path;

/// Supplies ordered skills, disabled identities, and discovery paths for explicit selection.
///
/// Implementations should preserve skill discovery order and expose the logical discovery path
/// associated with each canonical skill identity when one is available.
pub trait ExplicitSkillLookup {
    fn skills(&self) -> &[SkillMetadata];

    fn disabled_paths(&self) -> &HashSet<AbsolutePathBuf>;

    fn skill_discovery_path_for_path(&self, path: &AbsolutePathBuf) -> Option<&AbsolutePathBuf>;

    fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        !self.disabled_paths().contains(&skill.path_to_skills_md)
    }
}

/// Collect explicitly mentioned skills from structured and text mentions.
///
/// Structured `UserInput::Skill` selections are resolved first by path against
/// enabled skills. Text inputs are then scanned to extract `$skill-name` tokens, and we
/// iterate loaded skills in their existing order to preserve prior ordering semantics.
/// Explicit paths match either a skill's canonical identity or its logical discovery
/// path, and plain names are only used when the match is unambiguous.
///
/// Complexity: `O(T + (N_s + N_t) * S)` time, `O(S + M)` space, where:
/// `S` = number of skills, `T` = total text length, `N_s` = number of structured skill inputs,
/// `N_t` = number of text inputs, `M` = max mentions parsed from a single text input.
pub fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    loaded_skills: &impl ExplicitSkillLookup,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    let skill_name_counts =
        build_skill_name_counts(loaded_skills.skills(), loaded_skills.disabled_paths()).0;

    let selection_context = SkillSelectionContext {
        loaded_skills,
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

            let Some(skill) = selection_context
                .loaded_skills
                .skills()
                .iter()
                .find(|skill| {
                    skill.path_to_skills_md == path
                        || selection_context
                            .loaded_skills
                            .skill_discovery_path_for_path(&skill.path_to_skills_md)
                            .is_some_and(|discovery_path| discovery_path == &path)
                })
            else {
                continue;
            };

            if !selection_context.loaded_skills.is_skill_enabled(skill)
                || seen_paths.contains(&skill.path_to_skills_md)
            {
                continue;
            }

            seen_paths.insert(skill.path_to_skills_md.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
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
    loaded_skills: &'a dyn ExplicitSkillLookup,
    skill_name_counts: &'a HashMap<String, usize>,
    connector_slug_counts: &'a HashMap<String, usize>,
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

    let mention_skill_paths: HashSet<String> = mentions
        .paths()
        .filter(|path| {
            !matches!(
                tool_kind_for_path(path),
                ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
            )
        })
        .map(normalize_host_skill_path)
        .collect();

    for skill in selection_context.loaded_skills.skills() {
        if !selection_context.loaded_skills.is_skill_enabled(skill)
            || seen_paths.contains(&skill.path_to_skills_md)
        {
            continue;
        }

        let canonical_path = normalize_host_skill_path(&skill.path_to_skills_md.to_string_lossy());
        let matches_discovery_path = selection_context
            .loaded_skills
            .skill_discovery_path_for_path(&skill.path_to_skills_md)
            .is_some_and(|discovery_path| {
                mention_skill_paths.contains(&normalize_host_skill_path(
                    &discovery_path.to_string_lossy(),
                ))
            });
        if mention_skill_paths.contains(&canonical_path) || matches_discovery_path {
            seen_paths.insert(skill.path_to_skills_md.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
        }
    }

    for skill in selection_context.loaded_skills.skills() {
        if !selection_context.loaded_skills.is_skill_enabled(skill)
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

fn normalize_host_skill_path(path: &str) -> String {
    normalize_skill_path(path).replace('\\', "/")
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
