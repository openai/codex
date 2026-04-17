use async_trait::async_trait;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fmt;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
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
    pub is_symlink: bool,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemSandboxContext {
    pub sandbox_policy: SandboxPolicy,
    pub windows_sandbox_level: WindowsSandboxLevel,
    #[serde(default)]
    pub windows_sandbox_private_desktop: bool,
    #[serde(default)]
    pub use_legacy_landlock: bool,
    pub additional_permissions: Option<PermissionProfile>,
}

impl FileSystemSandboxContext {
    pub fn new(sandbox_policy: SandboxPolicy) -> Self {
        Self {
            sandbox_policy,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
            additional_permissions: None,
        }
    }

    pub fn should_run_in_sandbox(&self) -> bool {
        matches!(
            self.sandbox_policy,
            SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
        )
    }
}

pub type FileSystemResult<T> = io::Result<T>;

/// A single filesystem operation mode for an executor-bound path.
///
/// Construct this from [`ExecutorPath::unsandboxed`],
/// [`ExecutorPath::with_sandbox`], [`ExecutorPathRef::unsandboxed`], or
/// [`ExecutorPathRef::with_sandbox`] so call sites make their sandbox intent
/// visible at the operation boundary.
pub struct ExecutorPathAccess<'a> {
    file_system: &'a dyn ExecutorFileSystem,
    path: &'a AbsolutePathBuf,
    sandbox: Option<&'a FileSystemSandboxContext>,
}

impl<'a> ExecutorPathAccess<'a> {
    pub fn new(
        file_system: &'a dyn ExecutorFileSystem,
        path: &'a AbsolutePathBuf,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> Self {
        Self {
            file_system,
            path,
            sandbox,
        }
    }

    pub fn path(&self) -> &AbsolutePathBuf {
        self.path
    }

    pub fn display(&self) -> Display<'_> {
        self.path.display()
    }

    pub async fn read_file(&self) -> FileSystemResult<Vec<u8>> {
        self.file_system.read_file(self.path, self.sandbox).await
    }

    pub async fn read_file_text(&self) -> FileSystemResult<String> {
        self.file_system
            .read_file_text(self.path, self.sandbox)
            .await
    }

    pub async fn write_file(&self, contents: Vec<u8>) -> FileSystemResult<()> {
        self.file_system
            .write_file(self.path, contents, self.sandbox)
            .await
    }

    pub async fn create_directory(
        &self,
        create_directory_options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        self.file_system
            .create_directory(self.path, create_directory_options, self.sandbox)
            .await
    }

    pub async fn get_metadata(&self) -> FileSystemResult<FileMetadata> {
        self.file_system.get_metadata(self.path, self.sandbox).await
    }

    pub async fn metadata_if_exists(&self) -> FileSystemResult<Option<FileMetadata>> {
        metadata_if_exists(self.file_system, self.path, self.sandbox).await
    }

    pub async fn exists(&self) -> FileSystemResult<bool> {
        Ok(self.metadata_if_exists().await?.is_some())
    }

    pub async fn is_dir(&self) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists()
            .await?
            .is_some_and(|metadata| metadata.is_directory))
    }

    pub async fn is_file(&self) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists()
            .await?
            .is_some_and(|metadata| metadata.is_file))
    }

    pub async fn read_directory(&self) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        self.file_system
            .read_directory(self.path, self.sandbox)
            .await
    }

    pub async fn remove(&self, remove_options: RemoveOptions) -> FileSystemResult<()> {
        self.file_system
            .remove(self.path, remove_options, self.sandbox)
            .await
    }
}

/// An absolute path bound to the executor filesystem where it should be
/// resolved.
///
/// Use this when a path names a file on a specific executor. Keeping the
/// filesystem and path together avoids call sites that accidentally read a
/// remote/executor path through local process filesystem APIs.
#[derive(Clone)]
pub struct ExecutorPath {
    file_system: Arc<dyn ExecutorFileSystem>,
    path: AbsolutePathBuf,
}

impl ExecutorPath {
    pub fn new(file_system: Arc<dyn ExecutorFileSystem>, path: AbsolutePathBuf) -> Self {
        Self { file_system, path }
    }

    pub fn as_ref(&self) -> ExecutorPathRef<'_> {
        ExecutorPathRef::new(self.file_system.as_ref(), self.path.clone())
    }

    pub fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.path.to_path_buf()
    }

    pub fn display(&self) -> Display<'_> {
        self.path.display()
    }

    pub fn is_same_file_system(&self, file_system: &Arc<dyn ExecutorFileSystem>) -> bool {
        Arc::ptr_eq(&self.file_system, file_system)
    }

    pub fn into_path(self) -> AbsolutePathBuf {
        self.path
    }

    pub fn with_path(&self, path: AbsolutePathBuf) -> Self {
        Self {
            file_system: Arc::clone(&self.file_system),
            path,
        }
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        self.with_path(self.path.join(path))
    }

    pub fn parent(&self) -> Option<Self> {
        self.path.parent().map(|path| self.with_path(path))
    }

    pub fn ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        self.path.ancestors().map(|path| self.with_path(path))
    }

    pub fn unsandboxed(&self) -> ExecutorPathAccess<'_> {
        self.with_sandbox(None)
    }

    pub fn sandboxed<'a>(
        &'a self,
        sandbox: &'a FileSystemSandboxContext,
    ) -> ExecutorPathAccess<'a> {
        self.with_sandbox(Some(sandbox))
    }

    pub fn with_sandbox<'a>(
        &'a self,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorPathAccess<'a> {
        ExecutorPathAccess::new(self.file_system.as_ref(), &self.path, sandbox)
    }

    pub async fn read_file(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        self.with_sandbox(sandbox).read_file().await
    }

    pub async fn read_file_text(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<String> {
        self.with_sandbox(sandbox).read_file_text().await
    }

    pub async fn get_metadata(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        self.with_sandbox(sandbox).get_metadata().await
    }

    pub async fn metadata_if_exists(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Option<FileMetadata>> {
        self.with_sandbox(sandbox).metadata_if_exists().await
    }

    pub async fn exists(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self.metadata_if_exists(sandbox).await?.is_some())
    }

    pub async fn is_dir(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists(sandbox)
            .await?
            .is_some_and(|metadata| metadata.is_directory))
    }

    pub async fn is_file(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists(sandbox)
            .await?
            .is_some_and(|metadata| metadata.is_file))
    }

    pub async fn read_directory(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        self.with_sandbox(sandbox).read_directory().await
    }
}

impl fmt::Debug for ExecutorPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorPath")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// Borrowed filesystem plus owned absolute path for short-lived executor path
/// operations.
#[derive(Clone)]
pub struct ExecutorPathRef<'a> {
    file_system: &'a dyn ExecutorFileSystem,
    path: AbsolutePathBuf,
}

impl<'a> ExecutorPathRef<'a> {
    pub fn new(file_system: &'a dyn ExecutorFileSystem, path: AbsolutePathBuf) -> Self {
        Self { file_system, path }
    }

    pub fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.path.to_path_buf()
    }

    pub fn display(&self) -> Display<'_> {
        self.path.display()
    }

    pub fn with_path(&self, path: AbsolutePathBuf) -> Self {
        Self {
            file_system: self.file_system,
            path,
        }
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        self.with_path(self.path.join(path))
    }

    pub fn parent(&self) -> Option<Self> {
        self.path.parent().map(|path| self.with_path(path))
    }

    pub fn ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        self.path.ancestors().map(|path| self.with_path(path))
    }

    pub fn unsandboxed(&self) -> ExecutorPathAccess<'_> {
        self.with_sandbox(None)
    }

    pub fn sandboxed<'b>(
        &'b self,
        sandbox: &'b FileSystemSandboxContext,
    ) -> ExecutorPathAccess<'b> {
        self.with_sandbox(Some(sandbox))
    }

    pub fn with_sandbox<'b>(
        &'b self,
        sandbox: Option<&'b FileSystemSandboxContext>,
    ) -> ExecutorPathAccess<'b> {
        ExecutorPathAccess::new(self.file_system, &self.path, sandbox)
    }

    pub async fn read_file(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        self.with_sandbox(sandbox).read_file().await
    }

    pub async fn read_file_text(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<String> {
        self.with_sandbox(sandbox).read_file_text().await
    }

    pub async fn get_metadata(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        self.with_sandbox(sandbox).get_metadata().await
    }

    pub async fn metadata_if_exists(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Option<FileMetadata>> {
        self.with_sandbox(sandbox).metadata_if_exists().await
    }

    pub async fn exists(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self.metadata_if_exists(sandbox).await?.is_some())
    }

    pub async fn is_dir(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists(sandbox)
            .await?
            .is_some_and(|metadata| metadata.is_directory))
    }

    pub async fn is_file(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<bool> {
        Ok(self
            .metadata_if_exists(sandbox)
            .await?
            .is_some_and(|metadata| metadata.is_file))
    }

    pub async fn read_directory(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        self.with_sandbox(sandbox).read_directory().await
    }
}

impl fmt::Debug for ExecutorPathRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorPathRef")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

async fn metadata_if_exists(
    file_system: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<Option<FileMetadata>> {
    match file_system.get_metadata(path, sandbox).await {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

#[async_trait]
pub trait ExecutorFileSystem: Send + Sync {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>>;

    /// Reads a file and decodes it as UTF-8 text.
    async fn read_file_text(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<String> {
        let bytes = self.read_file(path, sandbox).await?;
        String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    async fn write_file(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()>;

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()>;

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata>;

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()>;

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()>;
}
