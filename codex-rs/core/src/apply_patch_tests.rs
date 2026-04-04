use super::*;
use async_trait::async_trait;
use codex_exec_server::CopyOptions;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemOperationOptions;
use codex_exec_server::ReadDirectoryEntry;
use codex_protocol::protocol::SandboxPolicy;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use tempfile::tempdir;
use tokio::io;

#[test]
fn convert_apply_patch_maps_add_variant() {
    let tmp = tempdir().expect("tmp");
    let p = tmp.path().join("a.txt");
    // Create an action with a single Add change
    let action = ApplyPatchAction::new_add_for_test(&p, "hello".to_string());

    let got = convert_apply_patch_to_protocol(&action);

    assert_eq!(
        got.get(&p),
        Some(&FileChange::Add {
            content: "hello".to_string()
        })
    );
}

#[derive(Default)]
struct RecordingExecutorFileSystem {
    raw_reads: Mutex<Vec<PathBuf>>,
    option_reads: Mutex<Vec<FileSystemOperationOptions>>,
    raw_writes: Mutex<Vec<PathBuf>>,
    option_writes: Mutex<Vec<FileSystemOperationOptions>>,
    raw_creates: Mutex<Vec<PathBuf>>,
    option_creates: Mutex<Vec<FileSystemOperationOptions>>,
    raw_removes: Mutex<Vec<PathBuf>>,
    option_removes: Mutex<Vec<FileSystemOperationOptions>>,
}

#[async_trait]
impl ExecutorFileSystem for RecordingExecutorFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> io::Result<Vec<u8>> {
        self.raw_reads
            .lock()
            .expect("raw_reads lock")
            .push(path.as_path().to_path_buf());
        Ok(b"before\n".to_vec())
    }

    async fn read_file_with_options(
        &self,
        path: &AbsolutePathBuf,
        options: &FileSystemOperationOptions,
    ) -> io::Result<Vec<u8>> {
        self.option_reads
            .lock()
            .expect("option_reads lock")
            .push(options.clone());
        self.read_file(path).await
    }

    async fn write_file(&self, path: &AbsolutePathBuf, _contents: Vec<u8>) -> io::Result<()> {
        self.raw_writes
            .lock()
            .expect("raw_writes lock")
            .push(path.as_path().to_path_buf());
        Ok(())
    }

    async fn write_file_with_options(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        options: &FileSystemOperationOptions,
    ) -> io::Result<()> {
        self.option_writes
            .lock()
            .expect("option_writes lock")
            .push(options.clone());
        self.write_file(path, contents).await
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        _options: CreateDirectoryOptions,
    ) -> io::Result<()> {
        self.raw_creates
            .lock()
            .expect("raw_creates lock")
            .push(path.as_path().to_path_buf());
        Ok(())
    }

    async fn create_directory_with_options(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
        fs_options: &FileSystemOperationOptions,
    ) -> io::Result<()> {
        self.option_creates
            .lock()
            .expect("option_creates lock")
            .push(fs_options.clone());
        self.create_directory(path, options).await
    }

    async fn get_metadata(&self, _path: &AbsolutePathBuf) -> io::Result<FileMetadata> {
        Err(io::Error::other("unused"))
    }

    async fn read_directory(&self, _path: &AbsolutePathBuf) -> io::Result<Vec<ReadDirectoryEntry>> {
        Err(io::Error::other("unused"))
    }

    async fn remove(&self, path: &AbsolutePathBuf, _options: RemoveOptions) -> io::Result<()> {
        self.raw_removes
            .lock()
            .expect("raw_removes lock")
            .push(path.as_path().to_path_buf());
        Ok(())
    }

    async fn remove_with_options(
        &self,
        path: &AbsolutePathBuf,
        options: RemoveOptions,
        fs_options: &FileSystemOperationOptions,
    ) -> io::Result<()> {
        self.option_removes
            .lock()
            .expect("option_removes lock")
            .push(fs_options.clone());
        self.remove(path, options).await
    }

    async fn copy(
        &self,
        _source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _options: CopyOptions,
    ) -> io::Result<()> {
        Err(io::Error::other("unused"))
    }
}

#[tokio::test]
async fn verification_filesystem_uses_default_operation_options() {
    let file_system = Arc::new(RecordingExecutorFileSystem::default());
    let cwd = PathBuf::from("/tmp/apply-patch-verification");
    let adapter =
        EnvironmentApplyPatchFileSystem::for_verification(file_system.clone(), cwd.clone());

    let content = adapter
        .read_text(Path::new("/tmp/apply-patch-verification.txt"))
        .await
        .expect("read through adapter");

    assert_eq!(content, "before\n");
    assert_eq!(
        file_system
            .option_reads
            .lock()
            .expect("option_reads lock")
            .as_slice(),
        [FileSystemOperationOptions {
            sandbox_policy: None,
            cwd: Some(AbsolutePathBuf::from_absolute_path(&cwd).expect("absolute cwd")),
        }]
    );
    assert_eq!(
        file_system
            .raw_reads
            .lock()
            .expect("raw_reads lock")
            .as_slice(),
        [PathBuf::from("/tmp/apply-patch-verification.txt")]
    );
}

#[tokio::test]
async fn apply_filesystem_uses_sandbox_options() {
    let file_system = Arc::new(RecordingExecutorFileSystem::default());
    let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
    let path = Path::new("/tmp/apply-patch-sandboxed/new.txt");
    let cwd = PathBuf::from("/tmp/apply-patch-sandboxed");
    let action = ApplyPatchAction::new_add_for_test(path, "hello".to_string());
    let adapter = EnvironmentApplyPatchFileSystem::for_apply(
        file_system.clone(),
        cwd.clone(),
        sandbox_policy.clone(),
    );

    codex_apply_patch::apply_action_with_fs(&action, &adapter)
        .await
        .expect("apply patch through adapter");

    assert_eq!(
        file_system
            .option_creates
            .lock()
            .expect("option_creates lock")
            .as_slice(),
        [FileSystemOperationOptions {
            sandbox_policy: Some(sandbox_policy.clone()),
            cwd: Some(AbsolutePathBuf::from_absolute_path(&cwd).expect("absolute cwd")),
        }]
    );
    assert_eq!(
        file_system
            .option_writes
            .lock()
            .expect("option_writes lock")
            .as_slice(),
        [FileSystemOperationOptions {
            sandbox_policy: Some(sandbox_policy),
            cwd: Some(
                AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/apply-patch-sandboxed"))
                    .expect("absolute cwd")
            ),
        }]
    );
}
