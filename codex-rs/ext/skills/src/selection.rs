use std::collections::HashSet;

use codex_core_skills::injection::extract_tool_mentions;
use codex_protocol::user_input::UserInput;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;

const SKILL_PATH_PREFIX: &str = "skill://";

pub(crate) fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    catalog: &SkillCatalog,
) -> Vec<SkillCatalogEntry> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    let mut blocked_plain_names = HashSet::new();

    for input in inputs {
        match input {
            UserInput::Skill { name, path } => {
                blocked_plain_names.insert(name.clone());
                select_by_path(catalog, &path.to_string_lossy(), &mut seen, &mut selected);
            }
            UserInput::Mention { name, path } if path_is_skill(path) => {
                blocked_plain_names.insert(name.clone());
                select_by_path(catalog, path, &mut seen, &mut selected);
            }
            UserInput::Text { .. } | UserInput::Image { .. } | UserInput::LocalImage { .. } => {}
            UserInput::Mention { .. } => {}
            _ => {}
        }
    }

    for input in inputs {
        let UserInput::Text { text, .. } = input else {
            continue;
        };

        let mentions = extract_tool_mentions(text);
        for path in mentions.paths() {
            if path_is_skill(path) {
                select_by_path(
                    catalog,
                    normalize_skill_path(path),
                    &mut seen,
                    &mut selected,
                );
            }
        }
        for name in mentions.plain_names() {
            if blocked_plain_names.contains(name) {
                continue;
            }
            if let Some(entry) = catalog
                .entries
                .iter()
                .find(|entry| entry.enabled && entry.name == name)
            {
                push_selected(entry, &mut seen, &mut selected);
            }
        }
    }

    selected
}

fn select_by_path(
    catalog: &SkillCatalog,
    path: &str,
    seen: &mut HashSet<SkillCatalogEntryKey>,
    selected: &mut Vec<SkillCatalogEntry>,
) {
    let normalized_path = normalize_skill_path(path);
    for entry in catalog.entries.iter().filter(|entry| entry.enabled) {
        if entry_matches_path(entry, normalized_path) {
            push_selected(entry, seen, selected);
        }
    }
}

fn push_selected(
    entry: &SkillCatalogEntry,
    seen: &mut HashSet<SkillCatalogEntryKey>,
    selected: &mut Vec<SkillCatalogEntry>,
) {
    let key = SkillCatalogEntryKey::from(entry);
    if seen.insert(key) {
        selected.push(entry.clone());
    }
}

fn entry_matches_path(entry: &SkillCatalogEntry, path: &str) -> bool {
    entry.main_prompt.as_str() == path
        || entry.id.0 == path
        || entry
            .display_path
            .as_deref()
            .is_some_and(|display_path| normalize_skill_path(display_path) == path)
}

fn path_is_skill(path: &str) -> bool {
    path.starts_with(SKILL_PATH_PREFIX)
        || path
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|file_name| file_name.eq_ignore_ascii_case("SKILL.md"))
}

fn normalize_skill_path(path: &str) -> &str {
    path.strip_prefix(SKILL_PATH_PREFIX).unwrap_or(path)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SkillCatalogEntryKey {
    authority: SkillAuthority,
    package: SkillPackageId,
}

impl From<&SkillCatalogEntry> for SkillCatalogEntryKey {
    fn from(entry: &SkillCatalogEntry) -> Self {
        Self {
            authority: entry.authority.clone(),
            package: entry.id.clone(),
        }
    }
}
