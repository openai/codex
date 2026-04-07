use async_trait::async_trait;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub is_directory: bool,
    pub is_file: bool,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

pub type FileSystemResult<T> = io::Result<T>;

#[async_trait]
pub trait ExecutorFileSystem: Send + Sync {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>>;

    /// Reads a file and decodes it as UTF-8 text.
    async fn read_file_text(&self, path: &AbsolutePathBuf) -> FileSystemResult<String> {
        let bytes = self.read_file(path).await?;
        String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    async fn read_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<Vec<u8>>;

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()>;

    async fn write_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()>;

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()>;

    async fn create_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()>;

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata>;

    async fn get_metadata_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<FileMetadata>;

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn read_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()>;

    async fn remove_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()>;

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
        copy_options: CopyOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()>;
}
