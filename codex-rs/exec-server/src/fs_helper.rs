use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;
use serde::Deserialize;
use serde::Serialize;
use tokio::io;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::RemoveOptions;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryEntry;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileParams;
use crate::protocol::FsWriteFileResponse;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;
use crate::rpc::not_found;

pub const CODEX_FS_HELPER_ARG1: &str = "--codex-run-as-fs-helper";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "params")]
pub(crate) enum FsHelperRequest {
    #[serde(rename = "fs/readFile")]
    ReadFile(FsReadFileParams),
    #[serde(rename = "fs/writeFile")]
    WriteFile(FsWriteFileParams),
    #[serde(rename = "fs/createDirectory")]
    CreateDirectory(FsCreateDirectoryParams),
    #[serde(rename = "fs/getMetadata")]
    GetMetadata(FsGetMetadataParams),
    #[serde(rename = "fs/readDirectory")]
    ReadDirectory(FsReadDirectoryParams),
    #[serde(rename = "fs/remove")]
    Remove(FsRemoveParams),
    #[serde(rename = "fs/copy")]
    Copy(FsCopyParams),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "payload", rename_all = "camelCase")]
pub(crate) enum FsHelperResponse {
    Ok(FsHelperPayload),
    Error(JSONRPCErrorError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "response")]
pub(crate) enum FsHelperPayload {
    #[serde(rename = "fs/readFile")]
    ReadFile(FsReadFileResponse),
    #[serde(rename = "fs/writeFile")]
    WriteFile(FsWriteFileResponse),
    #[serde(rename = "fs/createDirectory")]
    CreateDirectory(FsCreateDirectoryResponse),
    #[serde(rename = "fs/getMetadata")]
    GetMetadata(FsGetMetadataResponse),
    #[serde(rename = "fs/readDirectory")]
    ReadDirectory(FsReadDirectoryResponse),
    #[serde(rename = "fs/remove")]
    Remove(FsRemoveResponse),
    #[serde(rename = "fs/copy")]
    Copy(FsCopyResponse),
}

impl FsHelperPayload {
    pub(crate) fn into_read_file(self) -> Option<FsReadFileResponse> {
        match self {
            Self::ReadFile(response) => Some(response),
            Self::WriteFile(_)
            | Self::CreateDirectory(_)
            | Self::GetMetadata(_)
            | Self::ReadDirectory(_)
            | Self::Remove(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_write_file(self) -> Option<FsWriteFileResponse> {
        match self {
            Self::WriteFile(response) => Some(response),
            Self::ReadFile(_)
            | Self::CreateDirectory(_)
            | Self::GetMetadata(_)
            | Self::ReadDirectory(_)
            | Self::Remove(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_create_directory(self) -> Option<FsCreateDirectoryResponse> {
        match self {
            Self::CreateDirectory(response) => Some(response),
            Self::ReadFile(_)
            | Self::WriteFile(_)
            | Self::GetMetadata(_)
            | Self::ReadDirectory(_)
            | Self::Remove(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_get_metadata(self) -> Option<FsGetMetadataResponse> {
        match self {
            Self::GetMetadata(response) => Some(response),
            Self::ReadFile(_)
            | Self::WriteFile(_)
            | Self::CreateDirectory(_)
            | Self::ReadDirectory(_)
            | Self::Remove(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_read_directory(self) -> Option<FsReadDirectoryResponse> {
        match self {
            Self::ReadDirectory(response) => Some(response),
            Self::ReadFile(_)
            | Self::WriteFile(_)
            | Self::CreateDirectory(_)
            | Self::GetMetadata(_)
            | Self::Remove(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_remove(self) -> Option<FsRemoveResponse> {
        match self {
            Self::Remove(response) => Some(response),
            Self::ReadFile(_)
            | Self::WriteFile(_)
            | Self::CreateDirectory(_)
            | Self::GetMetadata(_)
            | Self::ReadDirectory(_)
            | Self::Copy(_) => None,
        }
    }

    pub(crate) fn into_copy(self) -> Option<FsCopyResponse> {
        match self {
            Self::Copy(response) => Some(response),
            Self::ReadFile(_)
            | Self::WriteFile(_)
            | Self::CreateDirectory(_)
            | Self::GetMetadata(_)
            | Self::ReadDirectory(_)
            | Self::Remove(_) => None,
        }
    }
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
        FsHelperRequest::ReadFile(params) => {
            let data = file_system
                .read_file(&params.path)
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::ReadFile(FsReadFileResponse {
                data_base64: STANDARD.encode(data),
            }))
        }
        FsHelperRequest::WriteFile(params) => {
            let bytes = STANDARD.decode(params.data_base64).map_err(|err| {
                invalid_request(format!(
                    "{} requires valid base64 dataBase64: {err}",
                    FS_WRITE_FILE_METHOD
                ))
            })?;
            file_system
                .write_file(&params.path, bytes)
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::WriteFile(FsWriteFileResponse {}))
        }
        FsHelperRequest::CreateDirectory(params) => {
            file_system
                .create_directory(
                    &params.path,
                    CreateDirectoryOptions {
                        recursive: params.recursive.unwrap_or(true),
                    },
                )
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::CreateDirectory(
                FsCreateDirectoryResponse {},
            ))
        }
        FsHelperRequest::GetMetadata(params) => {
            let metadata = file_system
                .get_metadata(&params.path)
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::GetMetadata(FsGetMetadataResponse {
                is_directory: metadata.is_directory,
                is_file: metadata.is_file,
                created_at_ms: metadata.created_at_ms,
                modified_at_ms: metadata.modified_at_ms,
            }))
        }
        FsHelperRequest::ReadDirectory(params) => {
            let entries = file_system
                .read_directory(&params.path)
                .await
                .map_err(map_fs_error)?
                .into_iter()
                .map(|entry| FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect();
            Ok(FsHelperPayload::ReadDirectory(FsReadDirectoryResponse {
                entries,
            }))
        }
        FsHelperRequest::Remove(params) => {
            file_system
                .remove(
                    &params.path,
                    RemoveOptions {
                        recursive: params.recursive.unwrap_or(true),
                        force: params.force.unwrap_or(true),
                    },
                )
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::Remove(FsRemoveResponse {}))
        }
        FsHelperRequest::Copy(params) => {
            file_system
                .copy(
                    &params.source_path,
                    &params.destination_path,
                    CopyOptions {
                        recursive: params.recursive,
                    },
                )
                .await
                .map_err(map_fs_error)?;
            Ok(FsHelperPayload::Copy(FsCopyResponse {}))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_requests_use_fs_method_names() -> serde_json::Result<()> {
        assert_eq!(
            serde_json::to_value(FsHelperRequest::WriteFile(FsWriteFileParams {
                path: std::env::current_dir()
                    .expect("cwd")
                    .join("file")
                    .as_path()
                    .try_into()
                    .expect("absolute path"),
                data_base64: String::new(),
                sandbox: None,
            }))?["operation"],
            FS_WRITE_FILE_METHOD,
        );
        Ok(())
    }
}
