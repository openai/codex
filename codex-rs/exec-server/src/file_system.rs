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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileSystemOperationOptions {
    pub sandbox_policy: Option<SandboxPolicy>,
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

    async fn read_file_with_options(
        &self,
        path: &AbsolutePathBuf,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<Vec<u8>> {
        self.read_file(path).await
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()>;

    async fn write_file_with_options(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<()> {
        self.write_file(path, contents).await
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()>;

    async fn create_directory_with_options(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<()> {
        self.create_directory(path, create_directory_options).await
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata>;

    async fn get_metadata_with_options(
        &self,
        path: &AbsolutePathBuf,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<FileMetadata> {
        self.get_metadata(path).await
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn read_directory_with_options(
        &self,
        path: &AbsolutePathBuf,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        self.read_directory(path).await
    }

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()>;

    async fn remove_with_options(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<()> {
        self.remove(path, remove_options).await
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()>;

    async fn copy_with_options(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
        _options: &FileSystemOperationOptions,
    ) -> FileSystemResult<()> {
        self.copy(source_path, destination_path, copy_options).await
    }
}
