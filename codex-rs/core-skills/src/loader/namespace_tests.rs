use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FindUpOptions;
use codex_exec_server::FindUpOutcome;
use codex_exec_server::FindUpRequest;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

use super::ResolvedSkillNamespace;
use super::SkillNamespaceResolver;

#[derive(Clone, Copy)]
enum BatchBehavior {
    Delegate,
    Error,
    MalformedCardinality,
}

struct RecordingFileSystem {
    inner: Arc<dyn ExecutorFileSystem>,
    batch_behavior: BatchBehavior,
    batch_requests: Mutex<Vec<Vec<FindUpRequest>>>,
    individual_find_up_count: AtomicUsize,
    read_paths: Mutex<Vec<PathUri>>,
}

impl RecordingFileSystem {
    fn new(batch_behavior: BatchBehavior) -> Self {
        Self {
            inner: Arc::clone(&LOCAL_FS),
            batch_behavior,
            batch_requests: Mutex::new(Vec::new()),
            individual_find_up_count: AtomicUsize::new(0),
            read_paths: Mutex::new(Vec::new()),
        }
    }

    fn batch_sizes(&self) -> Vec<usize> {
        self.batch_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(Vec::len)
            .collect()
    }

    fn individual_find_up_count(&self) -> usize {
        self.individual_find_up_count.load(Ordering::Acquire)
    }

    fn read_count(&self, path: &PathUri) -> usize {
        self.read_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|read_path| *read_path == path)
            .count()
    }
}

impl ExecutorFileSystem for RecordingFileSystem {
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
        self.read_paths
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
        self.inner.get_metadata(path, sandbox)
    }

    fn find_up<'a>(
        &'a self,
        start: &'a PathUri,
        options: &'a FindUpOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FindUpOutcome> {
        self.individual_find_up_count.fetch_add(1, Ordering::AcqRel);
        self.inner.find_up(start, options, sandbox)
    }

    fn find_up_batch<'a>(
        &'a self,
        requests: &'a [FindUpRequest],
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<FileSystemResult<FindUpOutcome>>> {
        self.batch_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(requests.to_vec());
        Box::pin(async move {
            match self.batch_behavior {
                BatchBehavior::Delegate => self.inner.find_up_batch(requests, sandbox).await,
                BatchBehavior::Error => Err(io::Error::other("batch unavailable")),
                BatchBehavior::MalformedCardinality => Ok(Vec::new()),
            }
        })
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        self.inner.read_directory(path, sandbox)
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

#[tokio::test]
async fn all_negative_namespace_lookups_use_one_batch_round() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let root = temp_dir.path().join("skills");
    let namespace_roots = [
        temp_dir.path().join("linked-a"),
        temp_dir.path().join("linked-b"),
    ];
    for path in std::iter::once(&root).chain(namespace_roots.iter()) {
        std::fs::create_dir_all(path).expect("create lookup root");
    }
    let root = path_uri(&root);
    let skill_path = root.join("sample/SKILL.md").expect("skill URI");
    let namespace_roots = namespace_roots
        .iter()
        .map(|path| path_uri(path))
        .collect::<HashSet<_>>();
    let file_system = RecordingFileSystem::new(BatchBehavior::Delegate);

    let resolver = SkillNamespaceResolver::discover(
        &file_system,
        &root,
        std::slice::from_ref(&skill_path),
        HashSet::new(),
        namespace_roots,
    )
    .await;

    assert_eq!(file_system.batch_sizes(), vec![3]);
    assert_eq!(file_system.individual_find_up_count(), 0);
    assert_eq!(
        resolver.for_skill(&root, &skill_path),
        &ResolvedSkillNamespace::Plain
    );
}

#[tokio::test]
async fn invalid_nearest_manifest_retries_only_that_lookup_in_a_second_round() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let outer = temp_dir.path().join("outer");
    let invalid = outer.join("invalid");
    write_manifest(&outer, r#"{"name":"outer"}"#);
    write_manifest(&invalid, "not json");
    let root = invalid.join("nested/skills");
    std::fs::create_dir_all(&root).expect("create skills root");
    let root = path_uri(&root);
    let skill_path = root.join("sample/SKILL.md").expect("skill URI");
    let file_system = RecordingFileSystem::new(BatchBehavior::Delegate);

    let resolver = SkillNamespaceResolver::discover(
        &file_system,
        &root,
        std::slice::from_ref(&skill_path),
        HashSet::new(),
        HashSet::new(),
    )
    .await;

    assert_eq!(file_system.batch_sizes(), vec![1, 1]);
    assert_eq!(file_system.individual_find_up_count(), 0);
    assert_eq!(
        resolver.for_skill(&root, &skill_path),
        &ResolvedSkillNamespace::Plugin("outer".to_string())
    );
}

#[tokio::test]
async fn duplicate_manifest_matches_share_manifest_validation() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let outer = temp_dir.path().join("outer");
    write_manifest(&outer, r#"{"name":"outer"}"#);
    let root = outer.join("skills");
    let linked_a = outer.join("linked-a");
    let linked_b = outer.join("linked-b");
    for path in [&root, &linked_a, &linked_b] {
        std::fs::create_dir_all(path).expect("create lookup root");
    }
    let root = path_uri(&root);
    let skill_path = root.join("sample/SKILL.md").expect("skill URI");
    let namespace_roots = [path_uri(&linked_a), path_uri(&linked_b)]
        .into_iter()
        .collect();
    let manifest = path_uri(&outer.join(".codex-plugin/plugin.json"));
    let file_system = RecordingFileSystem::new(BatchBehavior::Delegate);

    SkillNamespaceResolver::discover(
        &file_system,
        &root,
        std::slice::from_ref(&skill_path),
        HashSet::new(),
        namespace_roots,
    )
    .await;

    assert_eq!(file_system.batch_sizes(), vec![3]);
    assert_eq!(file_system.read_count(&manifest), 1);
}

#[tokio::test]
async fn batch_error_falls_back_to_individual_lookups() {
    assert_batch_failure_falls_back(BatchBehavior::Error).await;
}

#[tokio::test]
async fn malformed_batch_cardinality_falls_back_to_individual_lookups() {
    assert_batch_failure_falls_back(BatchBehavior::MalformedCardinality).await;
}

async fn assert_batch_failure_falls_back(batch_behavior: BatchBehavior) {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let outer = temp_dir.path().join("outer");
    let invalid = outer.join("invalid");
    write_manifest(&outer, r#"{"name":"outer"}"#);
    write_manifest(&invalid, "not json");
    let root = invalid.join("nested/skills");
    std::fs::create_dir_all(&root).expect("create skills root");
    let root = path_uri(&root);
    let skill_path = root.join("sample/SKILL.md").expect("skill URI");
    let fallback_file_system = RecordingFileSystem::new(batch_behavior);

    let fallback = SkillNamespaceResolver::discover(
        &fallback_file_system,
        &root,
        std::slice::from_ref(&skill_path),
        HashSet::new(),
        HashSet::new(),
    )
    .await;

    assert_eq!(
        fallback.for_skill(&root, &skill_path),
        &ResolvedSkillNamespace::Plugin("outer".to_string())
    );
    assert_eq!(fallback_file_system.batch_sizes(), vec![1]);
    assert_eq!(fallback_file_system.individual_find_up_count(), 2);
}

fn write_manifest(root: &Path, contents: &str) {
    let manifest = root.join(".codex-plugin/plugin.json");
    std::fs::create_dir_all(manifest.parent().expect("manifest parent"))
        .expect("create manifest parent");
    std::fs::write(manifest, contents).expect("write manifest");
}

fn path_uri(path: &Path) -> PathUri {
    PathUri::from_host_native_path(path).expect("path URI")
}
