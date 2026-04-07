use async_trait::async_trait;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Component;
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
    ) -> FileSystemResult<Vec<u8>> {
        enforce_read_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<()> {
        enforce_write_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<()> {
        enforce_write_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<FileMetadata> {
        enforce_read_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        enforce_read_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<()> {
        enforce_write_access(path, sandbox_policy)?;
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
    ) -> FileSystemResult<()> {
        enforce_read_access(source_path, sandbox_policy)?;
        enforce_write_access(destination_path, sandbox_policy)?;
        self.copy(source_path, destination_path, copy_options).await
    }
}

fn enforce_read_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
) -> FileSystemResult<()> {
    enforce_access(
        path,
        sandbox_policy,
        FileSystemSandboxPolicy::can_read_path_with_cwd,
        "read",
    )
}

fn enforce_write_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
) -> FileSystemResult<()> {
    enforce_access(
        path,
        sandbox_policy,
        FileSystemSandboxPolicy::can_write_path_with_cwd,
        "write",
    )
}

fn enforce_access(
    path: &AbsolutePathBuf,
    sandbox_policy: Option<&SandboxPolicy>,
    is_allowed: fn(&FileSystemSandboxPolicy, &Path, &Path) -> bool,
    access_kind: &str,
) -> FileSystemResult<()> {
    let Some(sandbox_policy) = sandbox_policy else {
        return Ok(());
    };
    let cwd = std::env::current_dir()
        .map_err(|err| io::Error::other(format!("failed to read current dir: {err}")))?;
    let file_system_policy = FileSystemSandboxPolicy::from(sandbox_policy);
    if is_allowed(&file_system_policy, path.as_path(), cwd.as_path()) {
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
    let destination = resolve_copy_destination_path(destination)?;
    Ok(destination.starts_with(&source))
}

fn resolve_copy_destination_path(path: &Path) -> io::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    let mut unresolved_suffix = Vec::new();
    let mut existing_path = normalized.as_path();
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
