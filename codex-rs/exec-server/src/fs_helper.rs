use codex_app_server_protocol::JSONRPCErrorError;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use tokio::io;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::RemoveOptions;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::ByteChunk;
use crate::protocol::FsReadDirectoryEntry;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;
use crate::rpc::not_found;

pub const CODEX_FS_HELPER_ARG0: &str = "codex-fs";
#[cfg(windows)]
pub const CODEX_FS_HELPER_ARG1: &str = "--codex-run-as-fs-helper";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase")]
pub(crate) enum FsHelperRequest {
    ReadFile {
        path: AbsolutePathBuf,
    },
    WriteFile {
        path: AbsolutePathBuf,
        data: ByteChunk,
    },
    CreateDirectory {
        path: AbsolutePathBuf,
        recursive: bool,
    },
    GetMetadata {
        path: AbsolutePathBuf,
    },
    ReadDirectory {
        path: AbsolutePathBuf,
    },
    Remove {
        path: AbsolutePathBuf,
        recursive: bool,
        force: bool,
    },
    Copy {
        source_path: AbsolutePathBuf,
        destination_path: AbsolutePathBuf,
        recursive: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "payload", rename_all = "camelCase")]
pub(crate) enum FsHelperResponse {
    Ok(FsHelperPayload),
    Error(JSONRPCErrorError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum FsHelperPayload {
    ReadFile {
        data: ByteChunk,
    },
    WriteFile,
    CreateDirectory,
    GetMetadata {
        is_directory: bool,
        is_file: bool,
        created_at_ms: i64,
        modified_at_ms: i64,
    },
    ReadDirectory {
        entries: Vec<FsReadDirectoryEntry>,
    },
    Remove,
    Copy,
}

pub(crate) fn unexpected_response(operation: &str) -> JSONRPCErrorError {
    internal_error(format!(
        "unexpected fs sandbox helper response for {operation}"
    ))
}

pub(crate) async fn run_direct_request(
    request: FsHelperRequest,
) -> Result<FsHelperPayload, JSONRPCErrorError> {
    let file_system = LocalFileSystem;
    match request {
        FsHelperRequest::ReadFile { path } => {
            let data = file_system.read_file(&path).await.map_err(map_fs_error)?;
            Ok(FsHelperPayload::ReadFile { data: data.into() })
        }
        FsHelperRequest::WriteFile { path, data } => {
            file_system
                .write_file(&path, data.into_inner())
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::WriteFile)
        }
        FsHelperRequest::CreateDirectory { path, recursive } => {
            file_system
                .create_directory(&path, CreateDirectoryOptions { recursive })
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::CreateDirectory)
        }
        FsHelperRequest::GetMetadata { path } => {
            let metadata = file_system
                .get_metadata(&path)
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::GetMetadata {
                is_directory: metadata.is_directory,
                is_file: metadata.is_file,
                created_at_ms: metadata.created_at_ms,
                modified_at_ms: metadata.modified_at_ms,
            })
        }
        FsHelperRequest::ReadDirectory { path } => {
            let entries = file_system
                .read_directory(&path)
                .await
                .map_err(map_fs_error)?
                .into_iter()
                .map(|entry| FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect();
            Ok(FsHelperPayload::ReadDirectory { entries })
        }
        FsHelperRequest::Remove {
            path,
            recursive,
            force,
        } => {
            file_system
                .remove(&path, RemoveOptions { recursive, force })
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::Remove)
        }
        FsHelperRequest::Copy {
            source_path,
            destination_path,
            recursive,
        } => {
            file_system
                .copy(&source_path, &destination_path, CopyOptions { recursive })
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::Copy)
        }
    }
}

fn map_fs_error(err: io::Error) -> JSONRPCErrorError {
    match err.kind() {
        io::ErrorKind::NotFound => not_found(err.to_string()),
        io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied => {
            invalid_request(err.to_string())
        }
        _ => internal_error(err.to_string()),
    }
}
