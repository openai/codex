use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::LoadedSkillRoot;
use codex_skills::SkillError;
use codex_skills::SkillRootSnapshotCache;
use codex_skills::SkillRootSnapshots;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use codex_utils_plugins::migrated_command_skills_root;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::Semaphore;

use crate::loader::MAX_CONCURRENT_ROOT_SCANS;
use crate::loader::io_test_support::ManifestMetadataBehavior;
use crate::loader::io_test_support::RecordingFileSystem;

use super::HostSkillRoot;
use super::load_and_merge_host_skill_roots;

#[derive(Default)]
struct TestPluginSkillSnapshots {
    snapshots: Mutex<HashMap<PluginSkillRoot, LoadedSkillRoot>>,
}

impl SkillRootSnapshotCache<PluginSkillRoot> for TestPluginSkillSnapshots {
    fn get(&self, root: &PluginSkillRoot) -> Option<LoadedSkillRoot> {
        self.snapshots.lock().unwrap().get(root).cloned()
    }

    fn insert(&self, root: PluginSkillRoot, snapshot: LoadedSkillRoot) {
        self.snapshots.lock().unwrap().insert(root, snapshot);
    }
}

fn write_skill(root: &AbsolutePathBuf, directory: &str, name: &str, description: &str) {
    let skill_dir = root.join(directory);
    fs::create_dir_all(&skill_dir).expect("create skill directory");
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n"),
    )
    .expect("write skill");
}

struct PluginSkillsFixture {
    _temp_dir: TempDir,
    plugin_path: AbsolutePathBuf,
    native_root: AbsolutePathBuf,
    migrated_root: AbsolutePathBuf,
    identity: PluginIdentity,
}

impl PluginSkillsFixture {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("temp dir");
        let plugin_path = AbsolutePathBuf::from_absolute_path_checked(temp_dir.path())
            .expect("absolute plugin path");
        let native_root = plugin_path.join("skills");
        let migrated_root = migrated_command_skills_root(&plugin_path);
        write_skill(&native_root, "review", "source-command-review", "native");
        write_skill(
            &migrated_root,
            "review",
            "source-command-review",
            "migrated duplicate",
        );
        write_skill(
            &migrated_root,
            "only",
            "source-command-only",
            "migrated only",
        );
        Self {
            _temp_dir: temp_dir,
            plugin_path,
            native_root,
            migrated_root,
            identity: PluginIdentity {
                plugin_id: "sample@test".to_string(),
                remote_plugin_id: Some("remote-sample".to_string()),
            },
        }
    }

    fn root(&self, path: AbsolutePathBuf) -> HostSkillRoot {
        HostSkillRoot::plugin(
            PluginSkillRoot {
                path,
                plugin_identity: self.identity.clone(),
                plugin_namespace: "sample".to_string(),
                plugin_root: self.plugin_path.clone(),
                discovery_mode: SkillDiscoveryMode::Recursive,
            },
            Arc::clone(&LOCAL_FS),
        )
    }

    fn roots(&self) -> Vec<HostSkillRoot> {
        [self.migrated_root.clone(), self.native_root.clone()]
            .into_iter()
            .map(|path| self.root(path))
            .collect()
    }
}

#[tokio::test]
async fn native_plugin_skill_replaces_migrated_command_with_the_same_name() {
    let fixture = PluginSkillsFixture::new();

    let outcome = load_and_merge_host_skill_roots(
        fixture.roots(),
        &Semaphore::new(2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.description.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("sample:source-command-only", "migrated only"),
            ("sample:source-command-review", "native"),
        ]
    );
}

#[tokio::test]
async fn nested_migrated_command_root_preserves_native_skill_precedence() {
    let fixture = PluginSkillsFixture::new();

    let outcome = load_and_merge_host_skill_roots(
        vec![
            fixture.root(fixture.migrated_root.join("review")),
            fixture.root(fixture.native_root.clone()),
            fixture.root(fixture.migrated_root.clone()),
        ],
        &Semaphore::new(3),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.description.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("sample:source-command-only", "migrated only"),
            ("sample:source-command-review", "native"),
        ]
    );
}

#[tokio::test]
async fn product_filtered_native_skill_does_not_hide_migrated_command() {
    let fixture = PluginSkillsFixture::new();
    let metadata_dir = fixture.native_root.join("review/agents");
    fs::create_dir_all(&metadata_dir).expect("create metadata directory");
    fs::write(
        metadata_dir.join("openai.yaml"),
        "policy:\n  products: [CHATGPT]\n",
    )
    .expect("write product policy");

    let outcome = load_and_merge_host_skill_roots(
        fixture.roots(),
        &Semaphore::new(2),
        Some(Product::Codex),
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.description.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("sample:source-command-only", "migrated only"),
            ("sample:source-command-review", "migrated duplicate"),
        ]
    );
}

#[tokio::test]
async fn overlapping_plugin_roots_keep_one_canonical_skill() {
    let fixture = PluginSkillsFixture::new();

    let outcome = load_and_merge_host_skill_roots(
        vec![
            fixture.root(fixture.native_root.clone()),
            fixture.root(fixture.native_root.join("review")),
        ],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["sample:source-command-review"]
    );
}

#[tokio::test]
async fn reuses_owner_managed_plugin_root_snapshots() {
    let fixture = PluginSkillsFixture::new();
    let snapshots = SkillRootSnapshots::new(Arc::new(TestPluginSkillSnapshots::default()));
    let first_outcome = load_and_merge_host_skill_roots(
        vec![fixture.root(fixture.native_root.clone())],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        Some(&snapshots),
    )
    .await;
    write_skill(
        &fixture.native_root,
        "review",
        "source-command-review",
        "updated",
    );

    let cached_outcome = load_and_merge_host_skill_roots(
        vec![fixture.root(fixture.native_root.clone())],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        Some(&snapshots),
    )
    .await;
    let refreshed_outcome = load_and_merge_host_skill_roots(
        vec![fixture.root(fixture.native_root.clone())],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(first_outcome.skills, cached_outcome.skills);
    assert_eq!(cached_outcome.skills[0].description, "native");
    assert_eq!(refreshed_outcome.skills[0].description, "updated");
    assert_eq!(
        cached_outcome.skills[0].remote_plugin_id.as_deref(),
        Some("remote-sample")
    );
}

#[tokio::test]
async fn preserves_plugin_skill_errors() {
    let fixture = PluginSkillsFixture::new();
    fs::write(
        fixture.native_root.join("review/SKILL.md"),
        "missing frontmatter",
    )
    .expect("write invalid plugin skill");

    let outcome = load_and_merge_host_skill_roots(
        vec![fixture.root(fixture.native_root.clone())],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors.len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn recognizes_symlinked_migrated_command_roots() {
    let fixture = PluginSkillsFixture::new();
    let alias = fixture.plugin_path.join("migrated-alias");
    std::os::unix::fs::symlink(fixture.migrated_root.as_path(), alias.as_path())
        .expect("create migrated root alias");

    let outcome = load_and_merge_host_skill_roots(
        vec![
            fixture.root(alias),
            fixture.root(fixture.native_root.clone()),
        ],
        &Semaphore::new(2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.description.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("sample:source-command-only", "migrated only"),
            ("sample:source-command-review", "native"),
        ]
    );
}

#[tokio::test]
async fn host_root_scans_wait_for_shared_capacity() {
    let temp = TempDir::new().expect("temp dir");
    let root = AbsolutePathBuf::from_absolute_path(temp.path()).expect("absolute skill root");
    let root_scan_slots = Semaphore::new(/*permits*/ 1);
    let held_slot = root_scan_slots
        .try_acquire()
        .expect("root scan slot should be available");
    let load = load_and_merge_host_skill_roots(
        vec![HostSkillRoot::host(
            root,
            SkillScope::Repo,
            Arc::clone(&LOCAL_FS),
        )],
        &root_scan_slots,
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    );
    tokio::pin!(load);

    assert!(futures::poll!(load.as_mut()).is_pending());
    drop(held_slot);
    let outcome = load.await;

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors, Vec::new());
}

#[tokio::test]
async fn duplicate_host_root_paths_preserve_first_root_scope() {
    let temp = TempDir::new().expect("temp dir");
    let root = AbsolutePathBuf::from_absolute_path(temp.path()).expect("absolute skill root");
    write_skill(&root, "demo", "demo", "Shared skill");

    let outcome = load_and_merge_host_skill_roots(
        vec![
            HostSkillRoot::host(root.clone(), SkillScope::Repo, Arc::clone(&LOCAL_FS)),
            HostSkillRoot::host(root.clone(), SkillScope::User, Arc::clone(&LOCAL_FS)),
        ],
        &Semaphore::new(/*permits*/ 2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.skills[0].scope, SkillScope::Repo);
    let canonical_root = AbsolutePathBuf::from_absolute_path(
        fs::canonicalize(root.as_path()).expect("canonicalize skill root"),
    )
    .expect("absolute canonical skill root");
    assert_eq!(
        outcome.skill_root_for_path(&outcome.skills[0].path_to_skills_md),
        Some(&canonical_root)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn deduplicated_symlinked_skill_preserves_first_discovery_path() {
    let source = TempDir::new().expect("source temp dir");
    let first = TempDir::new().expect("first root temp dir");
    let second = TempDir::new().expect("second root temp dir");
    let source_root =
        AbsolutePathBuf::from_absolute_path(source.path()).expect("absolute source root");
    let first_root =
        AbsolutePathBuf::from_absolute_path(first.path()).expect("absolute first root");
    let second_root =
        AbsolutePathBuf::from_absolute_path(second.path()).expect("absolute second root");
    write_skill(&source_root, "demo", "demo", "Shared skill");
    std::os::unix::fs::symlink(source.path().join("demo"), first.path().join("first-link"))
        .expect("create first skill directory symlink");
    std::os::unix::fs::symlink(
        source.path().join("demo"),
        second.path().join("second-link"),
    )
    .expect("create second skill directory symlink");

    let outcome = load_and_merge_host_skill_roots(
        vec![
            HostSkillRoot::host(first_root.clone(), SkillScope::Repo, Arc::clone(&LOCAL_FS)),
            HostSkillRoot::host(second_root, SkillScope::User, Arc::clone(&LOCAL_FS)),
        ],
        &Semaphore::new(/*permits*/ 2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(outcome.skills.len(), 1);
    let skill_path = &outcome.skills[0].path_to_skills_md;
    let canonical_first_root = AbsolutePathBuf::from_absolute_path(
        fs::canonicalize(first_root.as_path()).expect("canonicalize first skill root"),
    )
    .expect("absolute first skill root");
    assert_eq!(
        (
            outcome.skill_root_for_path(skill_path),
            outcome.skill_discovery_path_for_path(skill_path),
        ),
        (
            Some(&canonical_first_root),
            Some(&canonical_first_root.join("first-link/SKILL.md")),
        )
    );
}

#[tokio::test]
async fn merges_host_root_results_in_input_order_when_scans_finish_out_of_order() {
    const ROOT_COUNT: usize = MAX_CONCURRENT_ROOT_SCANS + 1;

    let temp = TempDir::new().expect("temp dir");
    let roots = (0..ROOT_COUNT)
        .map(|index| temp.path().join(format!("root-{index}")))
        .collect::<Vec<_>>();
    for root in &roots {
        fs::create_dir_all(root).expect("create skill root");
    }
    let first_skill = roots[0].join("broken/SKILL.md");
    let second_skill = roots[1].join("broken/SKILL.md");
    for path in [&first_skill, &second_skill] {
        fs::create_dir_all(path.parent().expect("skill parent"))
            .expect("create invalid skill directory");
        fs::write(path, "missing frontmatter").expect("write invalid skill");
    }

    let mut recording =
        RecordingFileSystem::new(LOCAL_FS.as_ref(), ManifestMetadataBehavior::Immediate);
    recording.blocked_walk_root = Some(PathUri::from_abs_path(
        &dunce::canonicalize(&roots[0])
            .expect("canonical blocked skill root")
            .abs(),
    ));
    let recording = Arc::new(recording);
    let file_system: Arc<dyn ExecutorFileSystem> = recording.clone();
    let host_roots = roots
        .iter()
        .map(|root| HostSkillRoot::host(root.clone().abs(), SkillScope::User, file_system.clone()))
        .collect::<Vec<_>>();
    let load = tokio::spawn(async move {
        load_and_merge_host_skill_roots(
            host_roots,
            &Semaphore::new(MAX_CONCURRENT_ROOT_SCANS),
            /*restriction_product*/ None,
            /*plugin_skill_snapshots*/ None,
        )
        .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), async {
        loop {
            let walk_started = recording.walk_started.notified();
            if recording.walks.load(Ordering::Acquire) == ROOT_COUNT {
                break;
            }
            walk_started.await;
        }
    })
    .await
    .expect("all host root scans should start despite the blocked first root");
    recording.blocked_walk_gate.add_permits(/*n*/ 1);
    let outcome = load.await.expect("host skill root loading should complete");

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(
        outcome.errors,
        vec![
            SkillError {
                path: dunce::canonicalize(first_skill)
                    .expect("canonical first invalid skill path")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
            SkillError {
                path: dunce::canonicalize(second_skill)
                    .expect("canonical second invalid skill path")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
        ]
    );
}
