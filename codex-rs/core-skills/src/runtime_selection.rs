use std::collections::HashSet;

use codex_protocol::user_input::UserInput;

use crate::injection::extract_tool_mentions;
use crate::runtime::SkillAuthority;
use crate::runtime::SkillCatalog;
use crate::runtime::SkillCatalogEntry;
use crate::runtime::SkillPackageId;
use crate::runtime::SkillSourceKind;

const SKILL_PATH_PREFIX: &str = "skill://";

/// Selects explicit skill mentions from one authority-aware runtime catalog.
///
/// Exact locators win. Plain names prefer executor, then orchestrator, then
/// host skills, and must be unique within the first matching authority kind.
/// Plain names that collide with another tool namespace are ignored.
/// A structured selection blocks a plain-name fallback even when its locator
/// no longer exists.
pub fn collect_runtime_skill_mentions(
    inputs: &[UserInput],
    catalog: &SkillCatalog,
    plain_name_conflicts: &HashSet<String>,
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
            _ => {}
        }
    }

    for input in inputs {
        let UserInput::Text { text, .. } = input else {
            continue;
        };
        let mentions = extract_tool_mentions(text);
        select_text_mentions(
            catalog,
            &mentions,
            &blocked_plain_names,
            plain_name_conflicts,
            &mut seen,
            &mut selected,
        );
    }

    selected
}

fn select_text_mentions(
    catalog: &SkillCatalog,
    mentions: &crate::injection::ToolMentions<'_>,
    blocked_plain_names: &HashSet<String>,
    plain_name_conflicts: &HashSet<String>,
    seen: &mut HashSet<SkillCatalogEntryKey>,
    selected: &mut Vec<SkillCatalogEntry>,
) {
    let mentioned_paths = mentions
        .paths()
        .filter(|path| path_is_skill(path))
        .map(normalize_skill_path)
        .collect::<HashSet<_>>();
    for entry in catalog.entries.iter().filter(|entry| entry.enabled) {
        if entry_paths(entry)
            .into_iter()
            .any(|path| mentioned_paths.contains(normalize_skill_path(path)))
        {
            push_selected(entry, seen, selected);
        }
    }

    let selected_names = mentions
        .plain_names()
        .filter(|name| !blocked_plain_names.contains(*name))
        .filter(|name| !plain_name_conflicts.contains(&name.to_ascii_lowercase()))
        .filter_map(|name| select_by_name(catalog, name))
        .map(SkillCatalogEntryKey::from)
        .collect::<HashSet<_>>();
    for entry in &catalog.entries {
        if selected_names.contains(&SkillCatalogEntryKey::from(entry)) {
            push_selected(entry, seen, selected);
        }
    }
}

fn select_by_path(
    catalog: &SkillCatalog,
    path: &str,
    seen: &mut HashSet<SkillCatalogEntryKey>,
    selected: &mut Vec<SkillCatalogEntry>,
) {
    let path = normalize_skill_path(path);
    for entry in catalog.entries.iter().filter(|entry| entry.enabled) {
        if entry_paths(entry)
            .into_iter()
            .any(|candidate| normalize_skill_path(candidate) == path)
        {
            push_selected(entry, seen, selected);
        }
    }
}

fn entry_paths(entry: &SkillCatalogEntry) -> [&str; 3] {
    [
        entry.main_prompt.as_str(),
        entry.id.0.as_str(),
        entry.rendered_path(),
    ]
}

fn select_by_name<'a>(catalog: &'a SkillCatalog, name: &str) -> Option<&'a SkillCatalogEntry> {
    for kind in [
        SkillSourceKind::Executor,
        SkillSourceKind::Orchestrator,
        SkillSourceKind::Host,
    ] {
        let mut matches = catalog
            .entries
            .iter()
            .filter(|entry| entry.enabled && entry.authority.kind == kind && entry.name == name);
        let first = matches.next();
        if first.is_some() {
            return first.filter(|_| matches.next().is_none());
        }
    }
    None
}

fn push_selected(
    entry: &SkillCatalogEntry,
    seen: &mut HashSet<SkillCatalogEntryKey>,
    selected: &mut Vec<SkillCatalogEntry>,
) {
    if seen.insert(SkillCatalogEntryKey::from(entry)) {
        selected.push(entry.clone());
    }
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

#[cfg(test)]
#[path = "runtime_selection_tests.rs"]
mod tests;
