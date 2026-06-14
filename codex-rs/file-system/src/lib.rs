use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AppPermissionProfile;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxEnforcement;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;

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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemSandboxContext<PathType = AbsolutePathBuf> {
    pub permissions: PermissionProfile<PathType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathUri>,
    pub windows_sandbox_level: WindowsSandboxLevel,
    #[serde(default)]
    pub windows_sandbox_private_desktop: bool,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

pub type AppFileSystemSandboxContext = FileSystemSandboxContext<AbsolutePathBuf>;
pub type ExecFileSystemSandboxContext = FileSystemSandboxContext<PathUri>;

impl AppFileSystemSandboxContext {
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
        let permissions = AppPermissionProfile::from_runtime_permissions_with_enforcement(
            SandboxEnforcement::from_legacy_sandbox_policy(&sandbox_policy),
            &file_system_sandbox_policy,
            NetworkSandboxPolicy::from(&sandbox_policy),
        );
        Ok(Self::from_permission_profile_with_cwd(permissions, cwd))
    }

    pub fn from_permission_profile(permissions: AppPermissionProfile) -> Self {
        Self::from_permissions_and_cwd(permissions, /*cwd*/ None)
    }

    pub fn from_permission_profile_with_cwd(
        permissions: AppPermissionProfile,
        cwd: PathUri,
    ) -> Self {
        Self::from_permissions_and_cwd(permissions, Some(cwd))
    }

    fn from_permissions_and_cwd(permissions: AppPermissionProfile, cwd: Option<PathUri>) -> Self {
        Self {
            permissions,
            cwd,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
            use_legacy_landlock: false,
        }
    }

    pub fn should_run_in_sandbox(&self) -> bool {
        let file_system_policy = self.permissions.file_system_sandbox_policy();
        matches!(file_system_policy.kind, FileSystemSandboxKind::Restricted)
            && !file_system_policy.has_full_disk_write_access()
    }

    pub fn into_path_uri(self) -> ExecFileSystemSandboxContext {
        FileSystemSandboxContext {
            permissions: map_permission_paths(self.permissions, |path| {
                Ok::<_, std::convert::Infallible>(PathUri::from_abs_path(&path))
            })
            .expect("mapping absolute paths to path URIs is infallible"),
            cwd: self.cwd,
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: self.windows_sandbox_private_desktop,
            use_legacy_landlock: self.use_legacy_landlock,
        }
    }
}

impl<PathType> FileSystemSandboxContext<PathType> {
    pub fn has_cwd_dependent_permissions(&self) -> bool {
        match &self.permissions {
            PermissionProfile::Managed {
                file_system: ManagedFileSystemPermissions::Restricted { entries, .. },
                ..
            } => entries.iter().any(|entry| match &entry.path {
                FileSystemPath::GlobPattern { pattern } => !Path::new(pattern).is_absolute(),
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { .. },
                } => true,
                FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => false,
            }),
            PermissionProfile::Managed {
                file_system: ManagedFileSystemPermissions::Unrestricted,
                ..
            }
            | PermissionProfile::Disabled
            | PermissionProfile::External { .. } => false,
        }
    }

    pub fn drop_cwd_if_unused(mut self) -> Self {
        if !self.has_cwd_dependent_permissions() {
            self.cwd = None;
        }
        self
    }
}

impl ExecFileSystemSandboxContext {
    pub fn try_into_native(self) -> io::Result<AppFileSystemSandboxContext> {
        Ok(FileSystemSandboxContext {
            permissions: map_permission_paths(self.permissions, |path| path.to_abs_path())?,
            cwd: self.cwd,
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: self.windows_sandbox_private_desktop,
            use_legacy_landlock: self.use_legacy_landlock,
        })
    }
}

fn map_permission_paths<SourcePath, TargetPath, Error>(
    permissions: PermissionProfile<SourcePath>,
    mut map_path: impl FnMut(SourcePath) -> Result<TargetPath, Error>,
) -> Result<PermissionProfile<TargetPath>, Error> {
    Ok(match permissions {
        PermissionProfile::Managed {
            file_system,
            network,
        } => PermissionProfile::Managed {
            file_system: match file_system {
                ManagedFileSystemPermissions::Restricted {
                    entries,
                    glob_scan_max_depth,
                } => ManagedFileSystemPermissions::Restricted {
                    entries: entries
                        .into_iter()
                        .map(|entry| {
                            Ok(FileSystemSandboxEntry {
                                path: match entry.path {
                                    FileSystemPath::Path { path } => FileSystemPath::Path {
                                        path: map_path(path)?,
                                    },
                                    FileSystemPath::GlobPattern { pattern } => {
                                        FileSystemPath::GlobPattern { pattern }
                                    }
                                    FileSystemPath::Special { value } => {
                                        FileSystemPath::Special { value }
                                    }
                                },
                                access: entry.access,
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                    glob_scan_max_depth,
                },
                ManagedFileSystemPermissions::Unrestricted => {
                    ManagedFileSystemPermissions::Unrestricted
                }
            },
            network,
        },
        PermissionProfile::Disabled => PermissionProfile::Disabled,
        PermissionProfile::External { network } => PermissionProfile::External { network },
    })
}

pub type FileSystemResult<T> = io::Result<T>;

/// Future returned by [`ExecutorFileSystem`] operations.
pub type ExecutorFileSystemFuture<'a, T> =
    Pin<Box<dyn Future<Output = FileSystemResult<T>> + Send + 'a>>;

/// Abstract filesystem access used by components that may operate locally or via
/// a remote environment.
pub trait ExecutorFileSystem: Send + Sync {
    /// Resolves a path within this filesystem.
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri>;

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>>;

    /// Reads a file and decodes it as UTF-8 text.
    fn read_file_text<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
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
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        create_directory_options: CreateDirectoryOptions,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata>;

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>>;

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        remove_options: RemoveOptions,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        copy_options: CopyOptions,
        sandbox: Option<&'a ExecFileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()>;
}
