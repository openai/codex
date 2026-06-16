use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::RwLock;

use codex_exec_server::ExecutorFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::SkillLoadOutcome;
use crate::loader::SkillRoot;
use crate::loader::SkillRootSnapshot;
use crate::loader::load_skill_root;
use crate::model::SkillFileSystemsByPath;

const MAX_CACHED_PLUGIN_SKILL_ROOTS: usize = 256;

#[derive(Clone)]
struct FileSystemIdentity(Arc<dyn ExecutorFileSystem>);

impl PartialEq for FileSystemIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FileSystemIdentity {}

impl Hash for FileSystemIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as *const ()).hash(state);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SkillRootCacheKey {
    path: AbsolutePathBuf,
    scope_rank: u8,
    file_system: FileSystemIdentity,
    plugin_id: String,
    plugin_namespace: String,
    plugin_root: AbsolutePathBuf,
}

impl SkillRootCacheKey {
    fn from_root(root: &SkillRoot) -> Option<Self> {
        Some(Self {
            path: root.path.clone(),
            scope_rank: scope_rank(root.scope),
            file_system: FileSystemIdentity(Arc::clone(&root.file_system)),
            plugin_id: root.plugin_id.clone()?,
            plugin_namespace: root.plugin_namespace.clone()?,
            plugin_root: root.plugin_root.clone()?,
        })
    }
}

/// Loads skill roots and reuses parsed plugin-root snapshots until explicitly invalidated.
///
/// Non-plugin roots are always loaded directly because their filesystem lifecycle is owned by the
/// environment or config layer that supplied them.
#[derive(Default)]
pub struct SkillRootLoader {
    plugin_root_cache: RwLock<HashMap<SkillRootCacheKey, SkillRootSnapshot>>,
}

impl SkillRootLoader {
    /// Loads and merges roots, reusing snapshots for roots owned by a plugin.
    pub async fn load_skills_from_roots<I>(&self, roots: I) -> SkillLoadOutcome
    where
        I: IntoIterator<Item = SkillRoot>,
    {
        let mut snapshots = Vec::new();
        for root in roots {
            let cache_key = SkillRootCacheKey::from_root(&root);
            let snapshot = match cache_key.as_ref().and_then(|key| self.cached_snapshot(key)) {
                Some(snapshot) => snapshot,
                None => {
                    let snapshot = load_skill_root(root).await;
                    if let Some(cache_key) = cache_key {
                        self.cache_snapshot(cache_key, snapshot.clone());
                    }
                    snapshot
                }
            };
            snapshots.push(snapshot);
        }

        merge_skill_root_snapshots(snapshots)
    }

    /// Invalidates every cached plugin-root snapshot.
    pub fn clear_cache(&self) {
        let mut cache = self
            .plugin_root_cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.clear();
    }

    fn cached_snapshot(&self, key: &SkillRootCacheKey) -> Option<SkillRootSnapshot> {
        match self.plugin_root_cache.read() {
            Ok(cache) => cache.get(key).cloned(),
            Err(err) => err.into_inner().get(key).cloned(),
        }
    }

    fn cache_snapshot(&self, key: SkillRootCacheKey, snapshot: SkillRootSnapshot) {
        let mut cache = self
            .plugin_root_cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache.len() < MAX_CACHED_PLUGIN_SKILL_ROOTS || cache.contains_key(&key) {
            cache.insert(key, snapshot);
        }
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

    outcome.skills.sort_by(|a, b| {
        scope_rank(a.scope)
            .cmp(&scope_rank(b.scope))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path_to_skills_md.cmp(&b.path_to_skills_md))
    });

    outcome
}

fn scope_rank(scope: codex_protocol::protocol::SkillScope) -> u8 {
    use codex_protocol::protocol::SkillScope;

    match scope {
        SkillScope::Repo => 0,
        SkillScope::User => 1,
        SkillScope::System => 2,
        SkillScope::Admin => 3,
    }
}

#[cfg(test)]
#[path = "root_loader_tests.rs"]
mod tests;
