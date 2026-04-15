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
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::RemoveOptions;
use std::io;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct FsApi {
    environment_manager: Arc<EnvironmentManager>,
}

impl FsApi {
    pub(crate) fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
        }
    }

    async fn file_system(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Arc<dyn ExecutorFileSystem>, JSONRPCErrorError> {
        let environment = self
            .environment_manager
            .environment(environment_id)
            .await
            .map_err(|err| invalid_request(format!("failed to resolve environment: {err}")))?;
        let environment =
            environment.ok_or_else(|| invalid_request("the selected environment is disabled"))?;
        Ok(environment.get_filesystem())
    }

    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        let bytes = file_system
            .read_file(&params.path, /*sandbox*/ None)
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
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        let bytes = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "fs/writeFile requires valid base64 dataBase64: {err}"
            ))
        })?;
        file_system
            .write_file(&params.path, bytes, /*sandbox*/ None)
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        file_system
            .create_directory(
                &params.path,
                CreateDirectoryOptions {
                    recursive: params.recursive.unwrap_or(true),
                },
                /*sandbox*/ None,
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCreateDirectoryResponse {})
    }

    pub(crate) async fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        let metadata = file_system
            .get_metadata(&params.path, /*sandbox*/ None)
            .await
            .map_err(map_fs_error)?;
        Ok(FsGetMetadataResponse {
            is_directory: metadata.is_directory,
            is_file: metadata.is_file,
            is_symlink: metadata.is_symlink,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        let entries = file_system
            .read_directory(&params.path, /*sandbox*/ None)
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
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        file_system
            .remove(
                &params.path,
                RemoveOptions {
                    recursive: params.recursive.unwrap_or(true),
                    force: params.force.unwrap_or(true),
                },
                /*sandbox*/ None,
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsRemoveResponse {})
    }

    pub(crate) async fn copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        let file_system = self.file_system(params.environment_id.as_deref()).await?;
        file_system
            .copy(
                &params.source_path,
                &params.destination_path,
                CopyOptions {
                    recursive: params.recursive,
                },
                /*sandbox*/ None,
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCopyResponse {})
    }
}

pub(crate) fn invalid_request(message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: None,
    }
}

pub(crate) fn map_fs_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::InvalidInput {
        invalid_request(err.to_string())
    } else {
        JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: err.to_string(),
            data: None,
        }
    }
}
