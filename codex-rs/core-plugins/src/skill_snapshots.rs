use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_skills::LoadedSkillRoot;
use codex_skills::SkillRootSnapshotCache;
use codex_skills::SkillRootSnapshots;
use codex_utils_plugins::PluginSkillRoot;

#[derive(Default)]
struct PluginSkillSnapshotCache {
    snapshots_by_root: Mutex<HashMap<PluginSkillRoot, LoadedSkillRoot>>,
}

impl SkillRootSnapshotCache<PluginSkillRoot> for PluginSkillSnapshotCache {
    fn get(&self, root: &PluginSkillRoot) -> Option<LoadedSkillRoot> {
        self.snapshots_by_root
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(root)
            .cloned()
    }

    fn insert(&self, root: PluginSkillRoot, snapshot: LoadedSkillRoot) {
        self.snapshots_by_root
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(root, snapshot);
    }
}

pub(crate) fn new_plugin_skill_snapshots() -> SkillRootSnapshots<PluginSkillRoot> {
    SkillRootSnapshots::new(Arc::new(PluginSkillSnapshotCache::default()))
}
