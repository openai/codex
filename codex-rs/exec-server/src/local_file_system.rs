use async_trait::async_trait;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::io;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;

const MAX_READ_FILE_BYTES: u64 = 512 * 1024 * 1024;

pub static LOCAL_FS: LazyLock<Arc<dyn ExecutorFileSystem>> =
    LazyLock::new(|| -> Arc<dyn ExecutorFileSystem> { Arc::new(LocalFileSystem) });

#[derive(Clone, Default)]
pub(crate) struct LocalFileSystem;

#[async_trait]
impl ExecutorFileSystem for LocalFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>> {
        let metadata = tokio::fs::metadata(path.as_path()).await?;
        if metadata.len() > MAX_READ_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("file is too large to read: limit is {MAX_READ_FILE_BYTES} bytes"),
            ));
        }
        tokio::fs::read(path.as_path()).await
    }

    async fn read_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<Vec<u8>> {
        enforce_read_access(path, sandbox_policy, sandbox_cwd)?;
        self.read_file(path).await
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()> {
        tokio::fs::write(path.as_path(), contents).await
    }

    async fn write_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()> {
        enforce_write_access(path, sandbox_policy, sandbox_cwd)?;
        self.write_file(path, contents).await
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        if options.recursive {
            tokio::fs::create_dir_all(path.as_path()).await?;
        } else {
            tokio::fs::create_dir(path.as_path()).await?;
        }
        Ok(())
    }

    async fn create_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()> {
        enforce_write_access(path, sandbox_policy, sandbox_cwd)?;
        self.create_directory(path, create_directory_options).await
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata> {
        let metadata = tokio::fs::metadata(path.as_path()).await?;
        Ok(FileMetadata {
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            created_at_ms: metadata.created().ok().map_or(0, system_time_to_unix_ms),
            modified_at_ms: metadata.modified().ok().map_or(0, system_time_to_unix_ms),
        })
    }

    async fn get_metadata_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<FileMetadata> {
        enforce_read_access(path, sandbox_policy, sandbox_cwd)?;
        self.get_metadata(path).await
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(path.as_path()).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = tokio::fs::metadata(entry.path()).await?;
            entries.push(ReadDirectoryEntry {
                file_name: entry.file_name().to_string_lossy().into_owned(),
                is_directory: metadata.is_dir(),
                is_file: metadata.is_file(),
            });
        }
        Ok(entries)
    }

    async fn read_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        enforce_read_access(path, sandbox_policy, sandbox_cwd)?;
        self.read_directory(path).await
    }

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()> {
        match tokio::fs::symlink_metadata(path.as_path()).await {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_dir() {
                    if options.recursive {
                        tokio::fs::remove_dir_all(path.as_path()).await?;
                    } else {
                        tokio::fs::remove_dir(path.as_path()).await?;
                    }
                } else {
                    tokio::fs::remove_file(path.as_path()).await?;
                }
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound && options.force => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn remove_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()> {
        enforce_write_access_preserving_leaf(path, sandbox_policy, sandbox_cwd)?;
        self.remove(path, remove_options).await
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()> {
        let source_path = source_path.to_path_buf();
        let destination_path = destination_path.to_path_buf();
        tokio::task::spawn_blocking(move || -> FileSystemResult<()> {
            let metadata = std::fs::symlink_metadata(source_path.as_path())?;
            let file_type = metadata.file_type();

            if file_type.is_dir() {
                if !options.recursive {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "fs/copy requires recursive: true when sourcePath is a directory",
                    ));
                }
                if destination_is_same_or_descendant_of_source(
                    source_path.as_path(),
                    destination_path.as_path(),
                )? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "fs/copy cannot copy a directory to itself or one of its descendants",
                    ));
                }
                copy_dir_recursive(source_path.as_path(), destination_path.as_path())?;
                return Ok(());
            }

            if file_type.is_symlink() {
                copy_symlink(source_path.as_path(), destination_path.as_path())?;
                return Ok(());
            }

            if file_type.is_file() {
                std::fs::copy(source_path.as_path(), destination_path.as_path())?;
                return Ok(());
            }

            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fs/copy only supports regular files, directories, and symlinks",
            ))
        })
        .await
        .map_err(|err| io::Error::other(format!("filesystem task failed: {err}")))?
    }

    async fn copy_with_sandbox_policy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
        sandbox_policy: Option<&SandboxPolicy>,
        sandbox_cwd: Option<&AbsolutePathBuf>,
    ) -> FileSystemResult<()> {
        enforce_copy_source_read_access(source_path, sandbox_policy, sandbox_cwd)?;
        enforce_write_access(destination_path, sandbox_policy, sandbox_cwd)?;
        self.copy(source_path, destination_path, copy_options).await
    }
}

fn enforce_read_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    sandbox_cwd: Option<&AbsolutePathBuf>,
) -> FileSystemResult<()> {
    enforce_access(
        path,
        sandbox_policy,
        sandbox_cwd,
        FileSystemSandboxPolicy::can_read_path_with_cwd,
        "read",
        AccessPathMode::ResolveAll,
    )
}

fn enforce_write_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    sandbox_cwd: Option<&AbsolutePathBuf>,
) -> FileSystemResult<()> {
    enforce_access(
        path,
        sandbox_policy,
        sandbox_cwd,
        FileSystemSandboxPolicy::can_write_path_with_cwd,
        "write",
        AccessPathMode::ResolveAll,
    )
}

fn enforce_write_access_preserving_leaf(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    sandbox_cwd: Option<&AbsolutePathBuf>,
) -> FileSystemResult<()> {
    enforce_access(
        path,
        sandbox_policy,
        sandbox_cwd,
        FileSystemSandboxPolicy::can_write_path_with_cwd,
        "write",
        AccessPathMode::PreserveLeaf,
    )
}

fn enforce_copy_source_read_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    sandbox_cwd: Option<&AbsolutePathBuf>,
) -> FileSystemResult<()> {
    let path_mode = match std::fs::symlink_metadata(path.as_path()) {
        Ok(metadata) if metadata.file_type().is_symlink() => AccessPathMode::PreserveLeaf,
        _ => AccessPathMode::ResolveAll,
    };
    enforce_access(
        path,
        sandbox_policy,
        sandbox_cwd,
        FileSystemSandboxPolicy::can_read_path_with_cwd,
        "read",
        path_mode,
    )
}

fn enforce_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    sandbox_cwd: Option<&AbsolutePathBuf>,
    is_allowed: fn(&FileSystemSandboxPolicy, &Path, &Path) -> bool,
    access_kind: &str,
    path_mode: AccessPathMode,
) -> FileSystemResult<()> {
    let Some(sandbox_policy) = sandbox_policy else {
        return Ok(());
    };
    let cwd = resolve_sandbox_cwd(sandbox_cwd)?;
    let resolved_path = resolve_path_for_access_check(path.as_path(), path_mode)?;
    let file_system_policy =
        canonicalize_file_system_policy_paths(FileSystemSandboxPolicy::from(sandbox_policy))?;
    if is_allowed(&file_system_policy, resolved_path.as_path(), cwd.as_path()) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "fs/{access_kind} is not permitted for path {}",
                path.as_path().display()
            ),
        ))
    }
}

#[derive(Clone, Copy)]
enum AccessPathMode {
    ResolveAll,
    PreserveLeaf,
}

fn copy_dir_recursive(source: &Path, target: &Path) -> io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &target_path)?;
        } else if file_type.is_symlink() {
            copy_symlink(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn destination_is_same_or_descendant_of_source(
    source: &Path,
    destination: &Path,
) -> io::Result<bool> {
    let source = std::fs::canonicalize(source)?;
    let destination = resolve_path_for_access_check(destination, AccessPathMode::ResolveAll)?;
    Ok(destination.starts_with(&source))
}

fn resolve_path_for_access_check(path: &Path, path_mode: AccessPathMode) -> io::Result<PathBuf> {
    match path_mode {
        AccessPathMode::ResolveAll => resolve_existing_path(path),
        AccessPathMode::PreserveLeaf => preserve_leaf_path_for_access_check(path),
    }
}

fn preserve_leaf_path_for_access_check(path: &Path) -> io::Result<PathBuf> {
    let Some(file_name) = path.file_name() else {
        return resolve_existing_path(path);
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    let mut resolved_parent = resolve_existing_path(parent)?;
    resolved_parent.push(file_name);
    Ok(resolved_parent)
}

fn resolve_existing_path(path: &Path) -> io::Result<PathBuf> {
    let mut unresolved_suffix = Vec::new();
    let mut existing_path = path;
    while !existing_path.exists() {
        let Some(file_name) = existing_path.file_name() else {
            break;
        };
        unresolved_suffix.push(file_name.to_os_string());
        let Some(parent) = existing_path.parent() else {
            break;
        };
        existing_path = parent;
    }

    let mut resolved = std::fs::canonicalize(existing_path)?;
    for file_name in unresolved_suffix.iter().rev() {
        resolved.push(file_name);
    }
    Ok(resolved)
}

fn resolve_sandbox_cwd(sandbox_cwd: Option<&AbsolutePathBuf>) -> io::Result<PathBuf> {
    let cwd = match sandbox_cwd {
        Some(cwd) => cwd.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|err| io::Error::other(format!("failed to read current dir: {err}")))?,
    };
    resolve_existing_path(cwd.as_path())
}

fn canonicalize_file_system_policy_paths(
    mut file_system_policy: FileSystemSandboxPolicy,
) -> io::Result<FileSystemSandboxPolicy> {
    for entry in &mut file_system_policy.entries {
        if let FileSystemPath::Path { path } = &mut entry.path {
            *path = canonicalize_absolute_path(path)?;
        }
    }
    Ok(file_system_policy)
}

fn canonicalize_absolute_path(path: &AbsolutePathBuf) -> io::Result<AbsolutePathBuf> {
    let resolved = resolve_existing_path(path.as_path())?;
    AbsolutePathBuf::from_absolute_path(resolved.as_path()).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path must stay absolute after canonicalization: {err}"),
        )
    })
}

fn copy_symlink(source: &Path, target: &Path) -> io::Result<()> {
    let link_target = std::fs::read_link(source)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&link_target, target)
    }
    #[cfg(windows)]
    {
        if symlink_points_to_directory(source)? {
            std::os::windows::fs::symlink_dir(&link_target, target)
        } else {
            std::os::windows::fs::symlink_file(&link_target, target)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = link_target;
        let _ = target;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "copying symlinks is unsupported on this platform",
        ))
    }
}

#[cfg(windows)]
fn symlink_points_to_directory(source: &Path) -> io::Result<bool> {
    use std::os::windows::fs::FileTypeExt;

    Ok(std::fs::symlink_metadata(source)?
        .file_type()
        .is_symlink_dir())
}

fn system_time_to_unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use codex_protocol::protocol::ReadOnlyAccess;
    use pretty_assertions::assert_eq;
    use std::os::unix::fs::symlink;

    fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
        match AbsolutePathBuf::try_from(path) {
            Ok(path) => path,
            Err(err) => panic!("absolute path: {err}"),
        }
    }

    fn read_only_sandbox_policy(readable_roots: Vec<PathBuf>) -> SandboxPolicy {
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: readable_roots.into_iter().map(absolute_path).collect(),
            },
            network_access: false,
        }
    }

    #[test]
    fn resolve_path_for_access_check_rejects_symlink_parent_dotdot_escape() -> io::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let allowed_dir = temp_dir.path().join("allowed");
        let outside_dir = temp_dir.path().join("outside");
        std::fs::create_dir_all(&allowed_dir)?;
        std::fs::create_dir_all(&outside_dir)?;
        symlink(&outside_dir, allowed_dir.join("link"))?;

        let resolved = resolve_path_for_access_check(
            allowed_dir
                .join("link")
                .join("..")
                .join("secret.txt")
                .as_path(),
            AccessPathMode::ResolveAll,
        )?;

        assert_eq!(resolved, temp_dir.path().join("secret.txt"));
        Ok(())
    }

    #[test]
    fn enforce_read_access_uses_explicit_sandbox_cwd() -> io::Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let workspace_dir = temp_dir.path().join("workspace");
        let other_dir = temp_dir.path().join("other");
        let note_path = workspace_dir.join("note.txt");
        std::fs::create_dir_all(&workspace_dir)?;
        std::fs::create_dir_all(&other_dir)?;
        std::fs::write(&note_path, "hello")?;

        let sandbox_policy = read_only_sandbox_policy(vec![]);
        let sandbox_cwd = absolute_path(workspace_dir);
        let other_cwd = absolute_path(other_dir);
        let note_path = absolute_path(note_path);

        enforce_read_access(&note_path, Some(&sandbox_policy), Some(&sandbox_cwd))?;

        let error = enforce_read_access(&note_path, Some(&sandbox_policy), Some(&other_cwd))
            .expect_err("read should be rejected outside provided cwd");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn symlink_points_to_directory_handles_dangling_directory_symlinks() -> io::Result<()> {
        use std::os::windows::fs::symlink_dir;

        let temp_dir = tempfile::TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let link_path = temp_dir.path().join("source-link");
        std::fs::create_dir(&source_dir)?;

        if symlink_dir(&source_dir, &link_path).is_err() {
            return Ok(());
        }

        std::fs::remove_dir(&source_dir)?;

        assert_eq!(symlink_points_to_directory(&link_path)?, true);
        Ok(())
    }
}
