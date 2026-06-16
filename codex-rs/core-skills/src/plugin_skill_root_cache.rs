use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use codex_utils_absolute_path::AbsolutePathBuf;
use futures::StreamExt;

use crate::SkillLoadOutcome;
use crate::loader::SkillRoot;
use crate::loader::SkillRootSnapshot;
use crate::loader::load_skill_root;
use crate::model::SkillFileSystemsByPath;

const MAX_CACHED_PLUGIN_SKILL_ROOTS: usize = 256;
const MAX_CONCURRENT_SKILL_ROOT_LOADS: usize = 8;

/// Shares parsed plugin skill-root snapshots between plugin and skill loading.
///
/// Non-plugin roots are always loaded directly because their filesystem lifecycle is owned by the
/// environment or config layer that supplied them.
#[derive(Default)]
pub struct PluginSkillRootCache {
    snapshots: RwLock<HashMap<AbsolutePathBuf, SkillRootSnapshot>>,
}

impl PluginSkillRootCache {
    /// Loads and merges roots, reusing snapshots for roots owned by a plugin.
    pub async fn load_skills_from_roots<I>(&self, roots: I) -> SkillLoadOutcome
    where
        I: IntoIterator<Item = SkillRoot>,
    {
        let snapshots = futures::stream::iter(roots)
            .map(|root| async move {
                // Plugin skill roots always use local filesystem and User scope, so the absolute
                // skill root path is sufficient to share their snapshot between plugin and skill
                // loading.
                let cache_key = root.plugin_root.as_ref().map(|_| root.path.clone());
                let cached_snapshot = cache_key
                    .as_ref()
                    .and_then(|root| match self.snapshots.read() {
                        Ok(cache) => cache.get(root).cloned(),
                        Err(err) => err.into_inner().get(root).cloned(),
                    });
                match cached_snapshot {
                    Some(snapshot) => snapshot,
                    None => {
                        let snapshot = load_skill_root(root).await;
                        if let Some(root) = cache_key {
                            let mut cache = self
                                .snapshots
                                .write()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if cache.len() < MAX_CACHED_PLUGIN_SKILL_ROOTS
                                || cache.contains_key(&root)
                            {
                                cache.insert(root, snapshot.clone());
                            }
                        }
                        snapshot
                    }
                }
            })
            .buffered(MAX_CONCURRENT_SKILL_ROOT_LOADS)
            .collect::<Vec<_>>()
            .await;

        merge_skill_root_snapshots(snapshots)
    }

    /// Invalidates every cached plugin-root snapshot.
    pub fn clear_cache(&self) {
        let mut cache = self
            .snapshots
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.clear();
    }
}

fn merge_skill_root_snapshots(snapshots: Vec<SkillRootSnapshot>) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    let mut skill_roots = Vec::new();
    let mut skill_root_by_path = HashMap::new();
    let mut file_systems_by_skill_path = HashMap::new();

    for snapshot in snapshots {
        let SkillRootSnapshot {
            root,
            skills,
            errors,
            file_system,
        } = snapshot;
        if !skills.is_empty() && !skill_roots.contains(&root) {
            skill_roots.push(root.clone());
        }
        for skill in &skills {
            skill_root_by_path
                .entry(skill.path_to_skills_md.clone())
                .or_insert_with(|| root.clone());
            file_systems_by_skill_path
                .entry(skill.path_to_skills_md.clone())
                .or_insert_with(|| Arc::clone(&file_system));
        }
        outcome.skills.extend(skills);
        outcome.errors.extend(errors);
    }

    let mut seen = HashSet::new();
    outcome
        .skills
        .retain(|skill| seen.insert(skill.path_to_skills_md.clone()));
    let retained_skill_paths = outcome
        .skills
        .iter()
        .map(|skill| skill.path_to_skills_md.clone())
        .collect::<HashSet<_>>();
    skill_root_by_path.retain(|path, _| retained_skill_paths.contains(path));
    let used_roots = skill_root_by_path.values().cloned().collect::<HashSet<_>>();
    skill_roots.retain(|root| used_roots.contains(root));
    file_systems_by_skill_path.retain(|path, _| retained_skill_paths.contains(path));
    outcome.skill_roots = skill_roots;
    outcome.skill_root_by_path = Arc::new(skill_root_by_path);
    outcome.file_systems_by_skill_path = SkillFileSystemsByPath::new(file_systems_by_skill_path);

    // The merged outcome includes non-plugin roots, so preserve the global scope ordering.
    outcome.skills.sort_by(|a, b| {
        merged_skill_scope_rank(a.scope)
            .cmp(&merged_skill_scope_rank(b.scope))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path_to_skills_md.cmp(&b.path_to_skills_md))
    });

    outcome
}

fn merged_skill_scope_rank(scope: codex_protocol::protocol::SkillScope) -> u8 {
    use codex_protocol::protocol::SkillScope;

    match scope {
        SkillScope::Repo => 0,
        SkillScope::User => 1,
        SkillScope::System => 2,
        SkillScope::Admin => 3,
    }
}

#[cfg(test)]
#[path = "plugin_skill_root_cache_tests.rs"]
mod tests;
