use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

use super::MAX_CONCURRENT_ROOT_SCANS;
use super::PluginSkillSnapshots;
use super::load_and_merge_skill_roots;
use crate::SkillError;
use crate::loader::SkillRoot;

const TEST_WAIT: Duration = Duration::from_secs(/*secs*/ 5);

struct ControlledFileSystem {
    inner: Arc<dyn ExecutorFileSystem>,
    walk_gate: Option<Arc<Semaphore>>,
    walk_paths: Mutex<Vec<PathUri>>,
    walks_started: AtomicUsize,
    walk_started: Notify,
    blocked_read_root: Option<PathUri>,
    blocked_read_gate: Arc<Semaphore>,
    observed_read_root: Option<PathUri>,
    observed_read: AtomicBool,
    observed_read_notify: Notify,
}

impl ControlledFileSystem {
    fn new(inner: Arc<dyn ExecutorFileSystem>) -> Self {
        Self {
            inner,
            walk_gate: None,
            walk_paths: Mutex::new(Vec::new()),
            walks_started: AtomicUsize::new(/*v*/ 0),
            walk_started: Notify::new(),
            blocked_read_root: None,
            blocked_read_gate: Arc::new(Semaphore::new(/*permits*/ 0)),
            observed_read_root: None,
            observed_read: AtomicBool::new(/*v*/ false),
            observed_read_notify: Notify::new(),
        }
    }

    fn with_walk_gate(mut self, walk_gate: Arc<Semaphore>) -> Self {
        self.walk_gate = Some(walk_gate);
        self
    }

    fn with_blocked_read_root(mut self, blocked_read_root: PathUri) -> Self {
        self.blocked_read_root = Some(blocked_read_root);
        self
    }

    fn with_observed_read_root(mut self, observed_read_root: PathUri) -> Self {
        self.observed_read_root = Some(observed_read_root);
        self
    }

    fn release_blocked_read(&self) {
        self.blocked_read_gate.add_permits(/*n*/ 1);
    }

    fn walks_started(&self) -> usize {
        self.walks_started.load(Ordering::Acquire)
    }

    fn walk_paths(&self) -> Vec<PathUri> {
        self.walk_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    async fn wait_for_walks(&self, expected: usize) {
        tokio::time::timeout(TEST_WAIT, async {
            loop {
                let notified = self.walk_started.notified();
                if self.walks_started() >= expected {
                    break;
                }
                notified.await;
            }
        })
        .await
        .expect("expected skill-root walks to start");
    }

    async fn wait_for_observed_read(&self) {
        tokio::time::timeout(TEST_WAIT, async {
            loop {
                let notified = self.observed_read_notify.notified();
                if self.observed_read.load(Ordering::Acquire) {
                    break;
                }
                notified.await;
            }
        })
        .await
        .expect("expected the unblocked skill root to read its skill");
    }
}

impl ExecutorFileSystem for ControlledFileSystem {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        self.inner.canonicalize(path, sandbox)
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        let should_block = path.basename().as_deref() == Some("SKILL.md")
            && self
                .blocked_read_root
                .as_ref()
                .is_some_and(|root| path.starts_with(root));
        let should_observe = path.basename().as_deref() == Some("SKILL.md")
            && self
                .observed_read_root
                .as_ref()
                .is_some_and(|root| path.starts_with(root));
        Box::pin(async move {
            if should_block {
                self.blocked_read_gate
                    .acquire()
                    .await
                    .expect("blocked read gate should remain open")
                    // Consume the permit so one release advances exactly one gated operation.
                    .forget();
            }
            let result = self.inner.read_file(path, sandbox).await;
            if should_observe {
                self.observed_read.store(/*val*/ true, Ordering::Release);
                self.observed_read_notify.notify_waiters();
            }
            result
        })
    }

    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        self.inner.read_file_stream(path, sandbox)
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.write_file(path, contents, sandbox)
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.create_directory(path, options, sandbox)
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        self.inner.get_metadata(path, sandbox)
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        self.inner.read_directory(path, sandbox)
    }

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        self.walk_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        self.walks_started.fetch_add(/*val*/ 1, Ordering::AcqRel);
        self.walk_started.notify_waiters();
        Box::pin(async move {
            if let Some(walk_gate) = &self.walk_gate {
                walk_gate
                    .acquire()
                    .await
                    .expect("walk gate should remain open")
                    // Consume the permit so one release advances exactly one gated operation.
                    .forget();
            }
            self.inner.walk(path, options, sandbox).await
        })
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.remove(path, options, sandbox)
    }

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner
            .copy(source_path, destination_path, options, sandbox)
    }
}

fn write_skill(root: &Path, directory: &str, contents: &str) -> PathBuf {
    let skill_path = root.join(directory).join("SKILL.md");
    fs::create_dir_all(
        skill_path
            .parent()
            .expect("skill path should have a parent"),
    )
    .expect("create skill directory");
    fs::write(&skill_path, contents).expect("write skill");
    skill_path
}

fn plain_root(
    path: &Path,
    scope: SkillScope,
    file_system: Arc<dyn ExecutorFileSystem>,
) -> SkillRoot {
    SkillRoot {
        path: path.to_path_buf().abs(),
        scope,
        file_system,
        plugin_id: None,
        plugin_namespace: Some("test".to_string()),
        plugin_root: None,
    }
}

fn plugin_root(
    skills_root: &Path,
    plugin_root: &Path,
    file_system: Arc<dyn ExecutorFileSystem>,
) -> SkillRoot {
    SkillRoot {
        path: skills_root.to_path_buf().abs(),
        scope: SkillScope::User,
        file_system,
        plugin_id: Some("demo@test".to_string()),
        plugin_namespace: Some("demo".to_string()),
        plugin_root: Some(plugin_root.to_path_buf().abs()),
    }
}

#[tokio::test]
async fn loads_roots_unordered_but_merges_errors_in_input_order() {
    const ROOT_COUNT: usize = MAX_CONCURRENT_ROOT_SCANS + 1;

    let temp = tempfile::tempdir().expect("tempdir");
    let roots = (0..ROOT_COUNT)
        .map(|index| {
            if index == 1 {
                temp.path().join("root-1").join(".agents").join("skills")
            } else {
                temp.path().join(format!("root-{index}"))
            }
        })
        .collect::<Vec<_>>();
    for root in &roots {
        fs::create_dir_all(root).expect("create root");
    }
    let first_skill = write_skill(&roots[0], "broken", "missing frontmatter");
    let second_skill = write_skill(&roots[1], "broken", "also missing frontmatter");

    let file_system = Arc::new(
        ControlledFileSystem::new(Arc::clone(&LOCAL_FS))
            .with_blocked_read_root(PathUri::from_host_native_path(&roots[0]).expect("first root"))
            .with_observed_read_root(
                PathUri::from_host_native_path(&roots[1]).expect("second root"),
            ),
    );
    let root_file_system: Arc<dyn ExecutorFileSystem> = file_system.clone();
    let skill_roots = roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            plain_root(
                root,
                if index == 0 {
                    SkillScope::Repo
                } else {
                    SkillScope::User
                },
                Arc::clone(&root_file_system),
            )
        })
        .collect::<Vec<_>>();

    let load = tokio::spawn(async move {
        load_and_merge_skill_roots(skill_roots, /*plugin_skill_snapshots*/ None).await
    });
    file_system.wait_for_observed_read().await;
    file_system.wait_for_walks(ROOT_COUNT).await;
    file_system.release_blocked_read();
    let outcome = load.await.expect("skill-root load should finish");

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(
        outcome.errors,
        vec![
            SkillError {
                path: dunce::canonicalize(first_skill)
                    .expect("canonical first skill")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
            SkillError {
                path: dunce::canonicalize(second_skill)
                    .expect("canonical second skill")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn starts_agents_skill_roots_before_other_pending_scans() {
    let temp = tempfile::tempdir().expect("tempdir");
    let default_roots = (0..MAX_CONCURRENT_ROOT_SCANS)
        .map(|index| temp.path().join(format!("default-{index}")))
        .collect::<Vec<_>>();
    let agents_roots = (0..MAX_CONCURRENT_ROOT_SCANS)
        .map(|index| {
            temp.path()
                .join(format!("repo-{index}"))
                .join(".agents")
                .join("skills")
        })
        .collect::<Vec<_>>();
    for root in default_roots.iter().chain(&agents_roots) {
        fs::create_dir_all(root).expect("create root");
    }

    let walk_gate = Arc::new(Semaphore::new(/*permits*/ 0));
    let file_system = Arc::new(
        ControlledFileSystem::new(Arc::clone(&LOCAL_FS)).with_walk_gate(Arc::clone(&walk_gate)),
    );
    let root_file_system: Arc<dyn ExecutorFileSystem> = file_system.clone();
    let skill_roots = default_roots
        .iter()
        .chain(&agents_roots)
        .map(|root| plain_root(root, SkillScope::Repo, Arc::clone(&root_file_system)))
        .collect::<Vec<_>>();

    let load = tokio::spawn(async move {
        load_and_merge_skill_roots(skill_roots, /*plugin_skill_snapshots*/ None).await
    });
    file_system.wait_for_walks(MAX_CONCURRENT_ROOT_SCANS).await;
    assert_eq!(
        file_system.walk_paths(),
        agents_roots
            .iter()
            .map(|root| PathUri::from_host_native_path(root).expect("agents root"))
            .collect::<Vec<_>>()
    );

    walk_gate.add_permits(MAX_CONCURRENT_ROOT_SCANS);
    file_system
        .wait_for_walks(MAX_CONCURRENT_ROOT_SCANS * 2)
        .await;
    walk_gate.add_permits(MAX_CONCURRENT_ROOT_SCANS);
    let outcome = load.await.expect("skill-root load should finish");
    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors, Vec::new());
}

#[tokio::test]
async fn limits_concurrent_root_scans() {
    const ROOT_COUNT: usize = MAX_CONCURRENT_ROOT_SCANS + 1;

    let temp = tempfile::tempdir().expect("tempdir");
    let roots = (0..ROOT_COUNT)
        .map(|index| temp.path().join(format!("root-{index}")))
        .collect::<Vec<_>>();
    for root in &roots {
        fs::create_dir_all(root).expect("create root");
    }

    let walk_gate = Arc::new(Semaphore::new(/*permits*/ 0));
    let file_system = Arc::new(
        ControlledFileSystem::new(Arc::clone(&LOCAL_FS)).with_walk_gate(Arc::clone(&walk_gate)),
    );
    let root_file_system: Arc<dyn ExecutorFileSystem> = file_system.clone();
    let skill_roots = roots
        .iter()
        .map(|root| plain_root(root, SkillScope::User, Arc::clone(&root_file_system)))
        .collect::<Vec<_>>();

    let load = tokio::spawn(async move {
        load_and_merge_skill_roots(skill_roots, /*plugin_skill_snapshots*/ None).await
    });
    file_system.wait_for_walks(MAX_CONCURRENT_ROOT_SCANS).await;
    assert_eq!(file_system.walks_started(), MAX_CONCURRENT_ROOT_SCANS);

    walk_gate.add_permits(/*n*/ 1);
    file_system.wait_for_walks(ROOT_COUNT).await;
    assert_eq!(file_system.walks_started(), ROOT_COUNT);

    walk_gate.add_permits(MAX_CONCURRENT_ROOT_SCANS);
    let outcome = load.await.expect("skill-root load should finish");
    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors, Vec::new());
}

#[tokio::test]
async fn duplicate_plugin_roots_share_work_only_when_snapshots_are_available() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skills_root = temp.path().join("plugin").join(".agents").join("skills");
    write_skill(
        &skills_root,
        "demo",
        "---\nname: demo\ndescription: demo skill\n---\n",
    );
    let broken_skill = write_skill(&skills_root, "broken", "missing frontmatter");
    let default_root = temp.path().join("default-skills");
    let default_broken_skill = write_skill(&default_root, "broken", "default missing frontmatter");

    let cached_file_system = Arc::new(ControlledFileSystem::new(Arc::clone(&LOCAL_FS)));
    let cached_root_file_system: Arc<dyn ExecutorFileSystem> = cached_file_system.clone();
    let snapshots = PluginSkillSnapshots::for_plugin_load();
    let cached_outcome = load_and_merge_skill_roots(
        [
            plain_root(
                &default_root,
                SkillScope::User,
                Arc::clone(&cached_root_file_system),
            ),
            plugin_root(
                &skills_root,
                temp.path(),
                Arc::clone(&cached_root_file_system),
            ),
            plugin_root(
                &skills_root,
                temp.path(),
                Arc::clone(&cached_root_file_system),
            ),
        ],
        Some(&snapshots),
    )
    .await;
    assert_eq!(cached_file_system.walks_started(), 2);

    let uncached_file_system = Arc::new(ControlledFileSystem::new(Arc::clone(&LOCAL_FS)));
    let uncached_root_file_system: Arc<dyn ExecutorFileSystem> = uncached_file_system.clone();
    let uncached_outcome = load_and_merge_skill_roots(
        [
            plain_root(
                &default_root,
                SkillScope::User,
                Arc::clone(&uncached_root_file_system),
            ),
            plugin_root(
                &skills_root,
                temp.path(),
                Arc::clone(&uncached_root_file_system),
            ),
            plugin_root(
                &skills_root,
                temp.path(),
                Arc::clone(&uncached_root_file_system),
            ),
        ],
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    assert_eq!(uncached_file_system.walks_started(), 3);

    assert_eq!(cached_outcome.skills, uncached_outcome.skills);
    assert_eq!(cached_outcome.errors, uncached_outcome.errors);
    assert_eq!(cached_outcome.skills.len(), 1);
    let expected_error = SkillError {
        path: dunce::canonicalize(broken_skill)
            .expect("canonical broken skill")
            .abs(),
        message: "missing YAML frontmatter delimited by ---".to_string(),
    };
    let expected_default_error = SkillError {
        path: dunce::canonicalize(default_broken_skill)
            .expect("canonical default broken skill")
            .abs(),
        message: "missing YAML frontmatter delimited by ---".to_string(),
    };
    assert_eq!(
        cached_outcome.errors,
        vec![
            expected_default_error,
            expected_error.clone(),
            expected_error,
        ]
    );
}
