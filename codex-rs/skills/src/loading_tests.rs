use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::LoadedSkillRoot;
use super::LoadedSkills;
use super::SkillLoadFuture;
use super::SkillRootLoadRequest;
use super::SkillRootLoader;
use super::SkillRootSnapshotCache;
use super::SkillRootSnapshots;

#[derive(Default)]
struct TestSnapshotCache {
    snapshots: Mutex<HashMap<String, LoadedSkillRoot>>,
}

impl SkillRootSnapshotCache<String> for TestSnapshotCache {
    fn get(&self, root: &String) -> Option<LoadedSkillRoot> {
        self.snapshots.lock().unwrap().get(root).cloned()
    }

    fn insert(&self, root: String, snapshot: LoadedSkillRoot) {
        self.snapshots.lock().unwrap().insert(root, snapshot);
    }
}

struct TestSkillRootLoader;

impl SkillRootLoader<String> for TestSkillRootLoader {
    fn load_roots(
        &self,
        _request: SkillRootLoadRequest<String>,
    ) -> SkillLoadFuture<'_, LoadedSkills> {
        Box::pin(async { LoadedSkills::default() })
    }
}

#[test]
fn snapshot_handles_share_owner_managed_values_and_identity() {
    let snapshots = SkillRootSnapshots::new(Arc::new(TestSnapshotCache::default()));
    let cloned_snapshots = snapshots.clone();
    let separate_snapshots = SkillRootSnapshots::new(Arc::new(TestSnapshotCache::default()));
    let root = "plugin-root".to_string();
    let snapshot = LoadedSkillRoot {
        root: AbsolutePathBuf::try_from(std::env::temp_dir()).unwrap(),
        skills: Vec::new(),
        skill_discovery_path_by_path: Arc::new(HashMap::new()),
        errors: Vec::new(),
        is_agent_plugin: false,
    };

    snapshots.insert(root.clone(), snapshot.clone());

    assert_eq!(cloned_snapshots.get(&root), Some(snapshot));
    assert_eq!(separate_snapshots.get(&root), None);
    assert_eq!(snapshots, cloned_snapshots);
    assert_ne!(snapshots, separate_snapshots);
    assert_eq!(
        HashSet::from([snapshots, cloned_snapshots, separate_snapshots]).len(),
        2
    );
}

#[test]
fn skill_root_loader_supports_shared_trait_objects() {
    let loader: Arc<dyn SkillRootLoader<String>> = Arc::new(TestSkillRootLoader);
    let future = loader.load_roots(SkillRootLoadRequest {
        roots: vec!["plugin-root".to_string()],
        restriction_product: None,
        snapshots: None,
    });

    drop(future);
}
