use std::io;
use std::sync::Arc;

use base64::Engine as _;
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

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::Environment;
use crate::ExecutorFileSystem;
use crate::RemoveOptions;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;

#[derive(Clone)]
pub(crate) struct ExecServerFileSystem {
    file_system: Arc<dyn ExecutorFileSystem>,
}

impl Default for ExecServerFileSystem {
    fn default() -> Self {
        Self {
            file_system: Arc::new(Environment::default().get_filesystem()),
        }
    }
}

impl ExecServerFileSystem {
    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let bytes = self
            .file_system
            .read_file(&params.path)
            .await
            .map_err(map_fs_error)?;
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
        self.file_system
            .write_file(&params.path, bytes)
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        self.file_system
            .create_directory(
                &params.path,
                CreateDirectoryOptions {
                    recursive: params.recursive.unwrap_or(true),
                },
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCreateDirectoryResponse {})
    }

    pub(crate) async fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        let metadata = self
            .file_system
            .get_metadata(&params.path)
            .await
            .map_err(map_fs_error)?;
        Ok(FsGetMetadataResponse {
            is_directory: metadata.is_directory,
            is_file: metadata.is_file,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        let entries = self
            .file_system
            .read_directory(&params.path)
            .await
            .map_err(map_fs_error)?;
        Ok(FsReadDirectoryResponse {
            entries: entries
                .into_iter()
                .map(|entry| FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect(),
        })
    }

    pub(crate) async fn remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, JSONRPCErrorError> {
        self.file_system
            .remove(
                &params.path,
                RemoveOptions {
                    recursive: params.recursive.unwrap_or(true),
                    force: params.force.unwrap_or(true),
                },
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsRemoveResponse {})
    }

    pub(crate) async fn copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        self.file_system
            .copy(
                &params.source_path,
                &params.destination_path,
                CopyOptions {
                    recursive: params.recursive,
                },
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCopyResponse {})
    }
}

fn map_fs_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::InvalidInput {
        invalid_request(err.to_string())
    } else {
        internal_error(err.to_string())
    }
}
