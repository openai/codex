use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_exec_server::WriteFileOptions;
use codex_utils_path_uri::PathUri;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

#[derive(Clone, Copy)]
pub(super) enum ManifestMetadataBehavior {
    Immediate,
    WaitForSkillRead,
}

pub(super) struct RecordingFileSystem<'a> {
    inner: &'a dyn ExecutorFileSystem,
    read_files: Mutex<Vec<PathUri>>,
    metadata_files: Mutex<Vec<PathUri>>,
    pub(super) walks: AtomicUsize,
    manifest_metadata_behavior: ManifestMetadataBehavior,
    skill_read_started: AtomicBool,
    skill_read_started_notify: Notify,
    pub(super) blocked_walk_root: Option<PathUri>,
    pub(super) blocked_walk_gate: Semaphore,
    pub(super) walk_started: Notify,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FileSystemCalls {
    pub(super) walks: usize,
    pub(super) read_files: Vec<PathUri>,
    pub(super) metadata_files: Vec<PathUri>,
}

impl<'a> RecordingFileSystem<'a> {
    pub(super) fn new(
        inner: &'a dyn ExecutorFileSystem,
        manifest_metadata_behavior: ManifestMetadataBehavior,
    ) -> Self {
        Self {
            inner,
            read_files: Mutex::new(Vec::new()),
            metadata_files: Mutex::new(Vec::new()),
            walks: AtomicUsize::new(/*v*/ 0),
            manifest_metadata_behavior,
            skill_read_started: AtomicBool::new(/*v*/ false),
            skill_read_started_notify: Notify::new(),
            blocked_walk_root: None,
            blocked_walk_gate: Semaphore::new(/*permits*/ 0),
            walk_started: Notify::new(),
        }
    }

    pub(super) fn calls(&self) -> FileSystemCalls {
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
        options: ReadFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        self.read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        if path.basename().as_deref() == Some("SKILL.md") {
            self.skill_read_started.store(true, Ordering::Release);
            self.skill_read_started_notify.notify_waiters();
        }
        self.inner.read_file(path, options, sandbox)
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
        options: WriteFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.write_file(path, contents, options, sandbox)
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
        options: GetMetadataOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        self.metadata_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        if matches!(
            self.manifest_metadata_behavior,
            ManifestMetadataBehavior::WaitForSkillRead
        ) && path.basename().as_deref() == Some("plugin.json")
        {
            return Box::pin(async move {
                loop {
                    let notified = self.skill_read_started_notify.notified();
                    if self.skill_read_started.load(Ordering::Acquire) {
                        break;
                    }
                    notified.await;
                }
                self.inner.get_metadata(path, options, sandbox).await
            });
        }
        self.inner.get_metadata(path, options, sandbox)
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
        self.walks.fetch_add(/*val*/ 1, Ordering::AcqRel);
        self.walk_started.notify_waiters();
        if self.blocked_walk_root.as_ref() != Some(path) {
            return self.inner.walk(path, options, sandbox);
        }
        Box::pin(async move {
            self.blocked_walk_gate
                .acquire()
                .await
                .expect("blocked walk gate should remain open")
                .forget();
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
