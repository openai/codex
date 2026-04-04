use async_trait::async_trait;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use tokio::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateDirectoryOptions {
    pub recursive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveOptions {
    pub recursive: bool,
    pub force: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyOptions {
    pub recursive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub is_directory: bool,
    pub is_file: bool,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

pub type FileSystemResult<T> = io::Result<T>;

#[async_trait]
pub trait ExecutorFileSystem: Send + Sync {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>>;

    async fn read_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<Vec<u8>> {
        self.read_file(path).await
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()>;

    async fn write_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<()> {
        self.write_file(path, contents).await
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()>;

    async fn create_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<()> {
        self.create_directory(path, options).await
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata>;

    async fn get_metadata_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<FileMetadata> {
        self.get_metadata(path).await
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn read_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        self.read_directory(path).await
    }

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()>;

    async fn remove_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        options: RemoveOptions,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<()> {
        self.remove(path, options).await
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()>;

    async fn copy_with_sandbox_policy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
        _sandbox_policy: Option<&FileSystemSandboxPolicy>,
    ) -> FileSystemResult<()> {
        self.copy(source_path, destination_path, options).await
    }
}
