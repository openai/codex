mod find_up;

use bytes::Bytes;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxEnforcement;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathUri;
pub use find_up::FindUpErrorPolicy;
pub use find_up::find_nearest_ancestor_with_markers;
pub use find_up::find_nearest_native_ancestor_with_markers;
use futures::Stream;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

/// Maximum chunk size returned by [`ExecutorFileSystem::read_file_stream`].
pub const FILE_READ_CHUNK_SIZE: usize = 1024 * 1024;
const MAX_WALK_DEPTH: usize = 64;
const MAX_WALK_DIRECTORIES: usize = 10_000;
const MAX_WALK_ENTRIES: usize = 50_000;
const MAX_WALK_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const WALK_RESPONSE_ITEM_OVERHEAD_BYTES: usize = 64;

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
    /// Size in bytes.
    pub size: u64,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

/// Bounds for a recursive filesystem walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkOptions {
    /// Maximum directory depth below the root that may be traversed.
    pub max_depth: usize,
    /// Maximum number of directories that may be traversed, including the root.
    pub max_directories: usize,
    /// Maximum number of directory entries that may be examined.
    pub max_entries: usize,
    /// Whether directory symlinks should be followed.
    pub follow_directory_symlinks: bool,
    /// Whether directories whose names start with `.` should be returned but not traversed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prune_hidden_directories: bool,
}

/// Type of a filesystem entry returned by a walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WalkEntryKind {
    Directory,
    File,
}

/// One entry returned by a walk.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkEntry {
    pub path: PathUri,
    pub kind: WalkEntryKind,
}

/// A descendant that could not be inspected during a walk.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkError {
    pub path: PathUri,
    pub message: String,
}

/// Entries and recoverable errors collected by a bounded walk.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkOutcome {
    pub entries: Vec<WalkEntry>,
    pub errors: Vec<WalkError>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecFileSystemPath {
    Path { path: PathUri },
    GeneratedDefaultPath { path: PathUri },
    GlobPattern { pattern: String },
    Special { value: FileSystemSpecialPath },
    GeneratedDefaultSpecial { value: FileSystemSpecialPath },
}

impl From<FileSystemPath> for ExecFileSystemPath {
    fn from(value: FileSystemPath) -> Self {
        match value {
            FileSystemPath::Path { path } => Self::Path {
                path: PathUri::from_abs_path(&path),
            },
            FileSystemPath::GeneratedDefaultPath { path } => Self::GeneratedDefaultPath {
                path: PathUri::from_abs_path(&path),
            },
            FileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            FileSystemPath::Special { value } => Self::Special { value },
            FileSystemPath::GeneratedDefaultSpecial { value } => {
                Self::GeneratedDefaultSpecial { value }
            }
        }
    }
}

impl TryFrom<ExecFileSystemPath> for FileSystemPath {
    type Error = io::Error;

    fn try_from(value: ExecFileSystemPath) -> Result<Self, Self::Error> {
        Ok(match value {
            ExecFileSystemPath::Path { path } => Self::Path {
                path: path.to_abs_path()?,
            },
            ExecFileSystemPath::GeneratedDefaultPath { path } => Self::GeneratedDefaultPath {
                path: path.to_abs_path()?,
            },
            ExecFileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            ExecFileSystemPath::Special { value } => Self::Special { value },
            ExecFileSystemPath::GeneratedDefaultSpecial { value } => {
                Self::GeneratedDefaultSpecial { value }
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecFileSystemSandboxEntry {
    pub path: ExecFileSystemPath,
    pub access: FileSystemAccessMode,
}

impl From<FileSystemSandboxEntry> for ExecFileSystemSandboxEntry {
    fn from(value: FileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access,
        }
    }
}

impl TryFrom<ExecFileSystemSandboxEntry> for FileSystemSandboxEntry {
    type Error = io::Error;

    fn try_from(value: ExecFileSystemSandboxEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            path: value.path.try_into()?,
            access: value.access,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecManagedFileSystemPermissions {
    Restricted {
        entries: Vec<ExecFileSystemSandboxEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        glob_scan_max_depth: Option<NonZeroUsize>,
    },
    Unrestricted,
}

impl From<ManagedFileSystemPermissions> for ExecManagedFileSystemPermissions {
    fn from(value: ManagedFileSystemPermissions) -> Self {
        match value {
            ManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries.into_iter().map(Into::into).collect(),
                glob_scan_max_depth,
            },
            ManagedFileSystemPermissions::Unrestricted => Self::Unrestricted,
        }
    }
}

impl TryFrom<ExecManagedFileSystemPermissions> for ManagedFileSystemPermissions {
    type Error = io::Error;

    fn try_from(value: ExecManagedFileSystemPermissions) -> Result<Self, Self::Error> {
        Ok(match value {
            ExecManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<io::Result<_>>()?,
                glob_scan_max_depth,
            },
            ExecManagedFileSystemPermissions::Unrestricted => Self::Unrestricted,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecPermissionProfile {
    Managed {
        file_system: ExecManagedFileSystemPermissions,
        network: NetworkSandboxPolicy,
    },
    Disabled,
    External {
        network: NetworkSandboxPolicy,
    },
}

impl From<PermissionProfile> for ExecPermissionProfile {
    fn from(value: PermissionProfile) -> Self {
        match value {
            PermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: file_system.into(),
                network,
            },
            PermissionProfile::Disabled => Self::Disabled,
            PermissionProfile::External { network } => Self::External { network },
        }
    }
}

impl TryFrom<ExecPermissionProfile> for PermissionProfile {
    type Error = io::Error;

    fn try_from(value: ExecPermissionProfile) -> Result<Self, Self::Error> {
        Ok(match value {
            ExecPermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: file_system.try_into()?,
                network,
            },
            ExecPermissionProfile::Disabled => Self::Disabled,
            ExecPermissionProfile::External { network } => Self::External { network },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemSandboxContext {
    pub permissions: ExecPermissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathUri>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_roots: Vec<PathUri>,
    pub windows_sandbox_level: WindowsSandboxLevel,
    #[serde(default)]
    pub windows_sandbox_private_desktop: bool,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

impl FileSystemSandboxContext {
    pub fn from_legacy_sandbox_policy(
        sandbox_policy: SandboxPolicy,
        cwd: PathUri,
    ) -> io::Result<Self> {
        // Legacy policy projection materializes native roots, so convert at the receiving-host
        // boundary while retaining the URI in the resulting sandbox context.
        let native_cwd = cwd.to_abs_path()?;
        let file_system_sandbox_policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
                &sandbox_policy,
                &native_cwd,
            );
        let permissions = PermissionProfile::from_runtime_permissions_with_enforcement(
            SandboxEnforcement::from_legacy_sandbox_policy(&sandbox_policy),
            &file_system_sandbox_policy,
            NetworkSandboxPolicy::from(&sandbox_policy),
        );
        Ok(Self::from_permission_profile_with_cwd(permissions, cwd))
    }

    pub fn from_permission_profile(permissions: PermissionProfile) -> Self {
        Self::from_permissions_and_cwd(permissions, /*cwd*/ None)
    }

    pub fn from_permission_profile_with_cwd(permissions: PermissionProfile, cwd: PathUri) -> Self {
        Self::from_permissions_and_cwd(permissions, Some(cwd))
    }

    fn from_permissions_and_cwd(permissions: PermissionProfile, cwd: Option<PathUri>) -> Self {
        let workspace_roots = cwd.iter().cloned().collect();
        Self {
            permissions: permissions.into(),
            cwd,
            workspace_roots,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
        }
    }

    pub fn should_run_in_sandbox(&self) -> bool {
        let Ok(permissions) = PermissionProfile::try_from(self.permissions.clone()) else {
            // A sandbox context for another host must not select the unsandboxed filesystem.
            return true;
        };
        let file_system_policy = permissions.file_system_sandbox_policy();
        matches!(file_system_policy.kind, FileSystemSandboxKind::Restricted)
            && !file_system_policy.has_full_disk_write_access()
    }

    pub fn has_cwd_dependent_permissions(&self) -> bool {
        match &self.permissions {
            ExecPermissionProfile::Managed {
                file_system: ExecManagedFileSystemPermissions::Restricted { entries, .. },
                ..
            } => entries.iter().any(|entry| match &entry.path {
                ExecFileSystemPath::GlobPattern { pattern } => !Path::new(pattern).is_absolute(),
                ExecFileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { .. },
                }
                | ExecFileSystemPath::GeneratedDefaultSpecial {
                    value: FileSystemSpecialPath::ProjectRoots { .. },
                } => true,
                ExecFileSystemPath::Path { .. }
                | ExecFileSystemPath::GeneratedDefaultPath { .. }
                | ExecFileSystemPath::Special { .. }
                | ExecFileSystemPath::GeneratedDefaultSpecial { .. } => false,
            }),
            ExecPermissionProfile::Managed {
                file_system: ExecManagedFileSystemPermissions::Unrestricted,
                ..
            }
            | ExecPermissionProfile::Disabled
            | ExecPermissionProfile::External { .. } => false,
        }
    }

    pub fn drop_cwd_if_unused(mut self) -> Self {
        if !self.has_cwd_dependent_permissions() {
            self.cwd = None;
            self.workspace_roots.clear();
        }
        self
    }
}

pub type FileSystemResult<T> = io::Result<T>;

/// Future returned by [`ExecutorFileSystem`] operations.
pub type ExecutorFileSystemFuture<'a, T> =
    Pin<Box<dyn Future<Output = FileSystemResult<T>> + Send + 'a>>;

/// Stream of immutable chunks read from an [`ExecutorFileSystem`].
pub struct FileSystemReadStream {
    inner: Pin<Box<dyn Stream<Item = FileSystemResult<Bytes>> + Send + 'static>>,
}

impl FileSystemReadStream {
    /// Wraps a filesystem byte stream.
    pub fn new(stream: impl Stream<Item = FileSystemResult<Bytes>> + Send + 'static) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl Stream for FileSystemReadStream {
    type Item = FileSystemResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

/// Abstract filesystem access used by components that may operate locally or via
/// a remote environment.
pub trait ExecutorFileSystem: Send + Sync {
    /// Resolves a path within this filesystem.
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri>;

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>>;

    /// Reads a file as a stream of chunks no larger than [`FILE_READ_CHUNK_SIZE`].
    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream>;

    /// Reads a file and decodes it as UTF-8 text.
    fn read_file_text<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, String> {
        Box::pin(async move {
            let bytes = self.read_file(path, sandbox).await?;
            String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        })
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        create_directory_options: CreateDirectoryOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata>;

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>>;

    /// Recursively lists descendants, optionally following directory symlinks.
    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        self.walk_via_directory_reads(path, options, sandbox)
    }

    /// Performs a bounded walk using the primitive filesystem operations.
    ///
    /// Implementations with an optimized walk transport can use this as a compatibility fallback.
    fn walk_via_directory_reads<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        Box::pin(walk_via_directory_reads(self, path, options, sandbox))
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        remove_options: RemoveOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        copy_options: CopyOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;
}

async fn walk_via_directory_reads<F: ExecutorFileSystem + ?Sized>(
    file_system: &F,
    root: &PathUri,
    options: WalkOptions,
    sandbox: Option<&FileSystemSandboxContext>,
) -> FileSystemResult<WalkOutcome> {
    if options.max_directories == 0 || options.max_entries == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "filesystem walk limits must be greater than zero",
        ));
    }
    if options.max_depth > MAX_WALK_DEPTH
        || options.max_directories > MAX_WALK_DIRECTORIES
        || options.max_entries > MAX_WALK_ENTRIES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "filesystem walk limits exceed maximums: depth={MAX_WALK_DEPTH}, directories={MAX_WALK_DIRECTORIES}, entries={MAX_WALK_ENTRIES}"
            ),
        ));
    }

    let root_metadata = file_system.get_metadata(root, sandbox).await?;
    if !root_metadata.is_directory
        || (root_metadata.is_symlink && !options.follow_directory_symlinks)
    {
        return Ok(WalkOutcome::default());
    }

    let root_identity = if options.follow_directory_symlinks {
        file_system.canonicalize(root, sandbox).await?
    } else {
        root.clone()
    };
    let mut outcome = WalkOutcome::default();
    let mut queue = VecDeque::from([(root.clone(), 0usize)]);
    let mut visited_directories = HashSet::from([root_identity]);
    let mut directory_count = 1usize;
    let mut entry_count = 0usize;
    let mut response_bytes = 0usize;

    while let Some((directory, depth)) = queue.pop_front() {
        let mut entries = match file_system.read_directory(&directory, sandbox).await {
            Ok(entries) => entries,
            Err(error) => {
                if !push_walk_error(
                    &mut outcome,
                    &mut response_bytes,
                    directory,
                    error.to_string(),
                ) {
                    return Ok(outcome);
                }
                continue;
            }
        };
        entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));

        for entry in entries {
            if entry_count == options.max_entries {
                outcome.truncated = true;
                return Ok(outcome);
            }
            entry_count += 1;

            let path = match directory.join(&entry.file_name) {
                Ok(path) => path,
                Err(error) => {
                    if !push_walk_error(
                        &mut outcome,
                        &mut response_bytes,
                        directory.clone(),
                        error.to_string(),
                    ) {
                        return Ok(outcome);
                    }
                    continue;
                }
            };
            let metadata = match file_system.get_metadata(&path, sandbox).await {
                Ok(metadata) => metadata,
                Err(error) => {
                    if !push_walk_error(&mut outcome, &mut response_bytes, path, error.to_string())
                    {
                        return Ok(outcome);
                    }
                    continue;
                }
            };
            if metadata.is_symlink && (!options.follow_directory_symlinks || !metadata.is_directory)
            {
                continue;
            }

            let kind = if metadata.is_directory {
                WalkEntryKind::Directory
            } else if metadata.is_file {
                WalkEntryKind::File
            } else {
                continue;
            };
            if !reserve_walk_response_bytes(
                &mut outcome,
                &mut response_bytes,
                path.to_string().len(),
            ) {
                return Ok(outcome);
            }
            outcome.entries.push(WalkEntry {
                path: path.clone(),
                kind,
            });

            if kind == WalkEntryKind::Directory && depth < options.max_depth {
                if options.prune_hidden_directories && entry.file_name.starts_with('.') {
                    continue;
                }
                let directory_identity = if options.follow_directory_symlinks {
                    match file_system.canonicalize(&path, sandbox).await {
                        Ok(path) => path,
                        Err(error) => {
                            if !push_walk_error(
                                &mut outcome,
                                &mut response_bytes,
                                path,
                                error.to_string(),
                            ) {
                                return Ok(outcome);
                            }
                            continue;
                        }
                    }
                } else {
                    path.clone()
                };
                if !visited_directories.insert(directory_identity) {
                    continue;
                }
                if directory_count == options.max_directories {
                    outcome.truncated = true;
                } else {
                    directory_count += 1;
                    queue.push_back((path, depth + 1));
                }
            }
        }
    }

    Ok(outcome)
}

fn push_walk_error(
    outcome: &mut WalkOutcome,
    response_bytes: &mut usize,
    path: PathUri,
    message: String,
) -> bool {
    let item_bytes = path.to_string().len().saturating_add(message.len());
    if !reserve_walk_response_bytes(outcome, response_bytes, item_bytes) {
        return false;
    }
    outcome.errors.push(WalkError { path, message });
    true
}

fn reserve_walk_response_bytes(
    outcome: &mut WalkOutcome,
    response_bytes: &mut usize,
    content_bytes: usize,
) -> bool {
    let item_bytes = content_bytes.saturating_add(WALK_RESPONSE_ITEM_OVERHEAD_BYTES);
    let Some(total_bytes) = response_bytes.checked_add(item_bytes) else {
        outcome.truncated = true;
        return false;
    };
    if total_bytes > MAX_WALK_RESPONSE_BYTES {
        outcome.truncated = true;
        return false;
    }
    *response_bytes = total_bytes;
    true
}
