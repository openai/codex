use std::fs;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;

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
use codex_exec_server::TextFilePrefix;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

use super::SKILL_FRONTMATTER_PREFIX_BYTES;
use super::read_skill_frontmatter_texts;

#[derive(Clone, Copy, Debug)]
enum BatchBehavior {
    Delegate,
    Error,
    WrongCardinality,
}

struct TestFileSystem {
    inner: Arc<dyn ExecutorFileSystem>,
    batch_behavior: BatchBehavior,
    full_reads: Mutex<Vec<PathUri>>,
}

impl TestFileSystem {
    fn new(batch_behavior: BatchBehavior) -> Self {
        Self {
            inner: Arc::clone(&LOCAL_FS),
            batch_behavior,
            full_reads: Mutex::new(Vec::new()),
        }
    }

    fn full_reads(&self) -> Vec<PathUri> {
        self.full_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ExecutorFileSystem for TestFileSystem {
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
        self.inner.read_file(path, sandbox)
    }

    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        self.inner.read_file_stream(path, sandbox)
    }

    fn read_file_text<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, String> {
        self.full_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        self.inner.read_file_text(path, sandbox)
    }

    fn read_text_prefixes_batch<'a>(
        &'a self,
        paths: &'a [PathUri],
        max_bytes: usize,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<io::Result<TextFilePrefix>>> {
        match self.batch_behavior {
            BatchBehavior::Delegate => self
                .inner
                .read_text_prefixes_batch(paths, max_bytes, sandbox),
            BatchBehavior::Error => {
                Box::pin(async { Err(io::Error::other("synthetic batch failure")) })
            }
            BatchBehavior::WrongCardinality => Box::pin(async { Ok(Vec::new()) }),
        }
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

fn write_file(root: &std::path::Path, name: &str, contents: &str) -> PathUri {
    let path = root.join(name);
    fs::write(&path, contents).expect("write test file");
    PathUri::from_abs_path(&AbsolutePathBuf::try_from(path).expect("absolute test path"))
}

fn unwrap_texts(results: Vec<io::Result<String>>) -> Vec<String> {
    results
        .into_iter()
        .map(|result| result.expect("read text"))
        .collect()
}

#[tokio::test]
async fn prefix_results_preserve_input_order() {
    let root = tempfile::tempdir().expect("tempdir");
    let first = format!(
        "---\nname: first\ndescription: first\n---\n{}",
        "a".repeat(4_000)
    );
    let second = "---\nname: second\ndescription: second\n---\nbody";
    let paths = vec![
        write_file(root.path(), "first.md", &first),
        write_file(root.path(), "second.md", second),
    ];
    let file_system = TestFileSystem::new(BatchBehavior::Delegate);

    let texts = unwrap_texts(read_skill_frontmatter_texts(&file_system, &paths).await);

    assert_eq!(texts[0].len(), SKILL_FRONTMATTER_PREFIX_BYTES);
    assert!(texts[0].starts_with("---\nname: first\n"));
    assert_eq!(texts[1], second);
    assert_eq!(file_system.full_reads(), Vec::<PathUri>::new());
}

#[tokio::test]
async fn incomplete_frontmatter_falls_back_to_full_read() {
    let root = tempfile::tempdir().expect("tempdir");
    let contents = format!(
        "---\nname: long\ndescription: {}\n---\nbody",
        "x".repeat(4_000)
    );
    let path = write_file(root.path(), "long.md", &contents);
    let file_system = TestFileSystem::new(BatchBehavior::Delegate);

    let texts =
        unwrap_texts(read_skill_frontmatter_texts(&file_system, std::slice::from_ref(&path)).await);

    assert_eq!(texts, vec![contents]);
    assert_eq!(file_system.full_reads(), vec![path]);
}

#[tokio::test]
async fn invalid_batch_results_fall_back_for_the_whole_chunk() {
    for behavior in [BatchBehavior::Error, BatchBehavior::WrongCardinality] {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = vec![
            write_file(root.path(), "first.md", "first"),
            write_file(root.path(), "second.md", "second"),
        ];
        let file_system = TestFileSystem::new(behavior);

        let texts = unwrap_texts(read_skill_frontmatter_texts(&file_system, &paths).await);

        assert_eq!(texts, vec!["first", "second"], "behavior: {behavior:?}");
        assert_eq!(file_system.full_reads(), paths, "behavior: {behavior:?}");
    }
}
