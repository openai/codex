use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCopyResponse;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsGetMetadataResponse;
use codex_app_server_protocol::FsReadDirectoryEntry;
use codex_app_server_protocol::FsReadDirectoryParams;
use codex_app_server_protocol::FsReadDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Clone, Default)]
pub(crate) struct FsApi;

impl FsApi {
    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let bytes = tokio::fs::read(params.path).await.map_err(map_io_error)?;
        Ok(FsReadFileResponse {
            data_base64: STANDARD.encode(bytes),
        })
    }

    pub(crate) async fn write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, JSONRPCErrorError> {
        let bytes = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "fs/writeFile requires valid base64 dataBase64: {err}"
            ))
        })?;
        tokio::fs::write(params.path, bytes)
            .await
            .map_err(map_io_error)?;
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        if params.recursive.unwrap_or(true) {
            tokio::fs::create_dir_all(params.path)
                .await
                .map_err(map_io_error)?;
        } else {
            tokio::fs::create_dir(params.path)
                .await
                .map_err(map_io_error)?;
        }
        Ok(FsCreateDirectoryResponse {})
    }

    pub(crate) async fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        let metadata = tokio::fs::metadata(params.path)
            .await
            .map_err(map_io_error)?;
        Ok(FsGetMetadataResponse {
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            created_at_ms: metadata.created().ok().map_or(0, system_time_to_unix_ms),
            modified_at_ms: metadata.modified().ok().map_or(0, system_time_to_unix_ms),
        })
    }

    pub(crate) async fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(params.path)
            .await
            .map_err(map_io_error)?;
        while let Some(entry) = read_dir.next_entry().await.map_err(map_io_error)? {
            let metadata = tokio::fs::metadata(entry.path())
                .await
                .map_err(map_io_error)?;
            entries.push(FsReadDirectoryEntry {
                file_name: entry.file_name().to_string_lossy().into_owned(),
                is_directory: metadata.is_dir(),
                is_file: metadata.is_file(),
            });
        }
        Ok(FsReadDirectoryResponse { entries })
    }

    pub(crate) async fn remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, JSONRPCErrorError> {
        let path = params.path.as_path();
        let recursive = params.recursive.unwrap_or(true);
        let force = params.force.unwrap_or(true);
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_dir() {
                    if recursive {
                        tokio::fs::remove_dir_all(path)
                            .await
                            .map_err(map_io_error)?;
                    } else {
                        tokio::fs::remove_dir(path).await.map_err(map_io_error)?;
                    }
                } else {
                    tokio::fs::remove_file(path).await.map_err(map_io_error)?;
                }
                Ok(FsRemoveResponse {})
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound && force => Ok(FsRemoveResponse {}),
            Err(err) => Err(map_io_error(err)),
        }
    }

    pub(crate) async fn copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        let FsCopyParams {
            source_path,
            destination_path,
            recursive,
        } = params;
        tokio::task::spawn_blocking(move || -> Result<(), JSONRPCErrorError> {
            let metadata =
                std::fs::symlink_metadata(source_path.as_path()).map_err(map_io_error)?;
            let file_type = metadata.file_type();

            if file_type.is_dir() {
                if !recursive {
                    return Err(invalid_request(
                        "fs/copy requires recursive: true when sourcePath is a directory",
                    ));
                }
                if destination_is_same_or_descendant_of_source(
                    source_path.as_path(),
                    destination_path.as_path(),
                )
                .map_err(map_io_error)?
                {
                    return Err(invalid_request(
                        "fs/copy cannot copy a directory to itself or one of its descendants",
                    ));
                }
                copy_dir_recursive(source_path.as_path(), destination_path.as_path())
                    .map_err(map_io_error)?;
                return Ok(());
            }

            if file_type.is_symlink() {
                copy_symlink(source_path.as_path(), destination_path.as_path())
                    .map_err(map_io_error)?;
                return Ok(());
            }

            if file_type.is_file() {
                std::fs::copy(source_path.as_path(), destination_path.as_path())
                    .map_err(map_io_error)?;
                return Ok(());
            }

            Err(invalid_request(
                "fs/copy only supports regular files, directories, and symlinks",
            ))
        })
        .await
        .map_err(map_join_error)??;
        Ok(FsCopyResponse {})
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> io::Result<()> {
    for entry in WalkDir::new(source) {
        let entry = entry.map_err(|err| {
            if let Some(io_err) = err.io_error() {
                io::Error::new(io_err.kind(), io_err.to_string())
            } else {
                io::Error::other(err.to_string())
            }
        })?;
        let relative_path = entry.path().strip_prefix(source).map_err(|err| {
            io::Error::other(format!(
                "failed to compute relative path for {} under {}: {err}",
                entry.path().display(),
                source.display()
            ))
        })?;
        let target_path = target.join(relative_path);
        let file_type = entry.file_type();

        if file_type.is_dir() {
            std::fs::create_dir_all(&target_path)?;
            continue;
        }

        if file_type.is_file() {
            std::fs::copy(entry.path(), &target_path)?;
            continue;
        }

        if file_type.is_symlink() {
            copy_symlink(entry.path(), &target_path)?;
            continue;
        }

        // For now ignore special files such as FIFOs, sockets, and device nodes during recursive copies.
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

pub(crate) fn invalid_request(message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: None,
    }
}

fn map_join_error(err: tokio::task::JoinError) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: format!("filesystem task failed: {err}"),
        data: None,
    }
}

pub(crate) fn map_io_error(err: io::Error) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
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
