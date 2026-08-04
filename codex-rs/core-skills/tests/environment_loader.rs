#![cfg(unix)]

use std::fs;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

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
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

struct RecordingFileSystem<'a> {
    inner: &'a dyn ExecutorFileSystem,
    read_files: Mutex<Vec<PathUri>>,
    metadata_files: Mutex<Vec<PathUri>>,
    walks: AtomicUsize,
}

#[derive(Debug, PartialEq, Eq)]
struct FileSystemCalls {
    walks: usize,
    read_files: Vec<PathUri>,
    metadata_files: Vec<PathUri>,
}

impl<'a> RecordingFileSystem<'a> {
    fn new(inner: &'a dyn ExecutorFileSystem) -> Self {
        Self {
            inner,
            read_files: Mutex::new(Vec::new()),
            metadata_files: Mutex::new(Vec::new()),
            walks: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> FileSystemCalls {
        let mut read_files = self
            .read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        read_files.sort_by_key(ToString::to_string);
        let mut metadata_files = self
            .metadata_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        metadata_files.sort_by_key(ToString::to_string);
        FileSystemCalls {
            walks: self.walks.load(Ordering::Relaxed),
            read_files,
            metadata_files,
        }
    }
}

impl ExecutorFileSystem for RecordingFileSystem<'_> {
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
        self.read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        self.inner.read_file(path, sandbox)
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
        self.metadata_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
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
        self.walks.fetch_add(1, Ordering::Relaxed);
        self.inner.walk(path, options, sandbox)
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

#[cfg(unix)]
#[tokio::test]
async fn host_loading_reuses_walk_inventory_for_symlinked_skill_pack() {
    use std::os::unix::fs::symlink;
    use std::sync::Arc;

    use codex_core_skills::SkillMetadata;
    use codex_core_skills::SkillPolicy;
    use codex_core_skills::loader::MAX_CONCURRENT_ROOT_SCANS;
    use codex_core_skills::loader::SkillRoot;
    use codex_core_skills::loader::load_skills_from_roots;
    use codex_protocol::protocol::SkillScope;
    use codex_utils_absolute_path::test_support::PathBufExt;

    let root = tempdir().expect("tempdir");
    let shared_plugin_root = tempdir().expect("tempdir");
    let manifest_path = shared_plugin_root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"linked"}"#).expect("manifest");

    let skills_root = shared_plugin_root.path().join("skills");
    for name in ["first", "second"] {
        let skill_path = skills_root.join(name).join("SKILL.md");
        fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
        fs::write(
            &skill_path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
    }
    let metadata_path = skills_root.join("first/agents/openai.yaml");
    fs::create_dir_all(metadata_path.parent().expect("metadata parent")).expect("metadata dir");
    fs::write(
        &metadata_path,
        "policy:\n  allow_implicit_invocation: false\n",
    )
    .expect("metadata");

    let host_root = root.path().join("skills");
    fs::create_dir_all(&host_root).expect("host skills dir");
    let linked_root = host_root.join("linked-plugin");
    symlink(&skills_root, &linked_root).expect("skill pack symlink");

    let recording = Arc::new(RecordingFileSystem::new(LOCAL_FS.as_ref()));
    let file_system: Arc<dyn ExecutorFileSystem> = recording.clone();
    let future = load_skills_from_roots(
        [SkillRoot {
            path: host_root.abs(),
            scope: SkillScope::User,
            file_system,
            plugin_identity: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: codex_utils_plugins::SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    );
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&future);
    let outcome = future.await;

    assert_eq!(outcome.errors, Vec::new());
    let first_skill_path = dunce::canonicalize(skills_root.join("first/SKILL.md"))
        .unwrap()
        .abs();
    let second_skill_path = dunce::canonicalize(skills_root.join("second/SKILL.md"))
        .unwrap()
        .abs();
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "linked:first".to_string(),
                description: "first skill.".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: Some(SkillPolicy {
                    allow_implicit_invocation: Some(false),
                    products: Vec::new(),
                }),
                path_to_skills_md: first_skill_path,
                scope: SkillScope::User,
                plugin_id: None,
                remote_plugin_id: None,
            },
            SkillMetadata {
                name: "linked:second".to_string(),
                description: "second skill.".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: second_skill_path,
                scope: SkillScope::User,
                plugin_id: None,
                remote_plugin_id: None,
            },
        ]
    );

    let calls = recording.calls();
    assert_eq!(calls.walks, 1);
    let linked_root = PathUri::from_host_native_path(linked_root).unwrap();
    assert!(
        calls
            .read_files
            .iter()
            .all(|path| !path.starts_with(&linked_root))
    );
    assert!(
        calls
            .metadata_files
            .iter()
            .all(|path| path.basename().as_deref() != Some("openai.yaml"))
    );
    let manifest_uri =
        PathUri::from_host_native_path(dunce::canonicalize(manifest_path).unwrap()).unwrap();
    assert_eq!(
        calls
            .metadata_files
            .iter()
            .filter(|path| **path == manifest_uri)
            .count(),
        1
    );
}
