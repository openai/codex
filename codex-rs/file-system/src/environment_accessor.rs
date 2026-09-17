//! Filesystem access bound to one environment configuration without exposing its authority.

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::ExecutorFileSystemFuture;
use crate::FileMetadata;
use crate::FileSystemReadStream;
use crate::FileSystemSandboxContext;
use crate::GetMetadataOptions;
use crate::ReadDirectoryEntry;
use crate::ReadFileOptions;
use crate::RemoveOptions;
use crate::WalkOptions;
use crate::WalkOutcome;
use crate::WriteFileOptions;
use codex_utils_path_uri::PathUri;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::Weak;

/// Filesystem operations authorized by the environment that supplied this borrowed accessor.
///
/// Callers cannot extract the underlying filesystem or choose another sandbox. They may retain
/// returned data, cache keys, and already-opened read streams after the accessor is gone.
pub trait EnvironmentAccess: Send + Sync {
    /// Identifies the filesystem and captured permissions without granting access to either.
    fn cache_key(&self) -> EnvironmentAccessKey;

    fn canonicalize<'a>(&'a self, path: &'a PathUri) -> ExecutorFileSystemFuture<'a, PathUri>;

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        options: ReadFileOptions,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>>;

    /// Opens an owned stream; subsequent reads do not borrow or reauthorize through the accessor.
    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream>;

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        options: WriteFileOptions,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        options: GetMetadataOptions,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata>;

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>>;

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome>;

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
    ) -> ExecutorFileSystemFuture<'a, ()>;
}

/// Convenience operations available to every implementation of [`EnvironmentAccess`].
pub trait EnvironmentAccessExt: EnvironmentAccess {
    fn read_file_text<'a>(
        &'a self,
        path: &'a PathUri,
        options: ReadFileOptions,
    ) -> ExecutorFileSystemFuture<'a, String>;
}

impl<T: EnvironmentAccess + ?Sized> EnvironmentAccessExt for T {
    fn read_file_text<'a>(
        &'a self,
        path: &'a PathUri,
        options: ReadFileOptions,
    ) -> ExecutorFileSystemFuture<'a, String> {
        Box::pin(async move {
            let bytes = self.read_file(path, options).await?;
            String::from_utf8(bytes)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        })
    }
}

/// Opaque identity for reusing data discovered with the same filesystem and permissions.
///
/// Holding this key keeps neither the underlying filesystem nor an accessor alive.
#[derive(Clone)]
pub struct EnvironmentAccessKey {
    file_system: Weak<dyn ExecutorFileSystem>,
    // Cache keys are cloned per root; share the permission and path lists instead of copying them.
    sandbox: Option<Arc<FileSystemSandboxContext>>,
}

impl fmt::Debug for EnvironmentAccessKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EnvironmentAccessKey(..)")
    }
}

impl PartialEq for EnvironmentAccessKey {
    fn eq(&self, other: &Self) -> bool {
        self.file_system.ptr_eq(&other.file_system) && self.sandbox == other.sandbox
    }
}

impl Eq for EnvironmentAccessKey {}

impl Hash for EnvironmentAccessKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file_system.as_ptr().cast::<()>().hash(state);
        // Sandbox contexts compare by value but are not Hash. Collisions for different policies
        // on the same filesystem are harmless; equality still compares the complete context.
        self.sandbox.is_some().hash(state);
    }
}

/// Borrows a filesystem and forwards every operation with its captured sandbox configuration.
/// The underlying filesystem remains responsible for choosing sandboxed or direct access.
pub struct FileSystemEnvironmentAccessor<'environment> {
    file_system: &'environment dyn ExecutorFileSystem,
    key: EnvironmentAccessKey,
}

impl<'environment> FileSystemEnvironmentAccessor<'environment> {
    /// Binds operations and cache identity to the captured environment permissions.
    pub fn new(
        file_system: &'environment Arc<dyn ExecutorFileSystem>,
        sandbox: FileSystemSandboxContext,
    ) -> Self {
        Self {
            file_system: file_system.as_ref(),
            key: EnvironmentAccessKey {
                file_system: Arc::downgrade(file_system),
                sandbox: Some(Arc::new(sandbox)),
            },
        }
    }

    /// Bypasses the filesystem sandbox. Use only for:
    ///
    /// 1. Tests.
    /// 2. Callers pending migration to sandboxed access.
    /// 3. Codex-internal operations that should never be sandboxed and that the model cannot trigger.
    ///
    /// Model-turn discovery must use the accessor supplied by its selected environment.
    pub fn unrestricted(file_system: &'environment Arc<dyn ExecutorFileSystem>) -> Self {
        Self {
            file_system: file_system.as_ref(),
            key: EnvironmentAccessKey {
                file_system: Arc::downgrade(file_system),
                sandbox: None,
            },
        }
    }
}

impl EnvironmentAccess for FileSystemEnvironmentAccessor<'_> {
    fn cache_key(&self) -> EnvironmentAccessKey {
        self.key.clone()
    }

    fn canonicalize<'a>(&'a self, path: &'a PathUri) -> ExecutorFileSystemFuture<'a, PathUri> {
        self.file_system
            .canonicalize(path, self.key.sandbox.as_deref())
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        options: ReadFileOptions,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        self.file_system
            .read_file(path, options, self.key.sandbox.as_deref())
    }

    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        self.file_system
            .read_file_stream(path, self.key.sandbox.as_deref())
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        options: WriteFileOptions,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.file_system
            .write_file(path, contents, options, self.key.sandbox.as_deref())
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.file_system
            .create_directory(path, options, self.key.sandbox.as_deref())
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        options: GetMetadataOptions,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        self.file_system
            .get_metadata(path, options, self.key.sandbox.as_deref())
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        self.file_system
            .read_directory(path, self.key.sandbox.as_deref())
    }

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        self.file_system
            .walk(path, options, self.key.sandbox.as_deref())
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.file_system
            .remove(path, options, self.key.sandbox.as_deref())
    }

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.file_system.copy(
            source_path,
            destination_path,
            options,
            self.key.sandbox.as_deref(),
        )
    }
}
