use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_skills::LoadedSkillRoot;
use codex_skills::SkillRootSnapshotCache;
use codex_skills::SkillRootSnapshots;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use codex_utils_plugins::migrated_command_skills_root;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::Semaphore;

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
