use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::fs_watch::FsWatchManager;
use crate::outgoing_message::ConnectionId;
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
use codex_app_server_protocol::FsReadFileCloseParams;
use codex_app_server_protocol::FsReadFileCloseResponse;
use codex_app_server_protocol::FsReadFileOpenParams;
use codex_app_server_protocol::FsReadFileOpenResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileReadParams;
use codex_app_server_protocol::FsReadFileReadResponse;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsReadFileStatParams;
use codex_app_server_protocol::FsReadFileStatResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsUnwatchParams;
use codex_app_server_protocol::FsUnwatchResponse;
use codex_app_server_protocol::FsWatchParams;
use codex_app_server_protocol::FsWatchResponse;
use codex_app_server_protocol::FsWriteFileCloseParams;
use codex_app_server_protocol::FsWriteFileCloseResponse;
use codex_app_server_protocol::FsWriteFileOpenParams;
use codex_app_server_protocol::FsWriteFileOpenResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::FsWriteFileWriteParams;
use codex_app_server_protocol::FsWriteFileWriteResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileHandleManager;
use codex_exec_server::RemoveOptions;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct FsRequestProcessor {
    environment_manager: Arc<EnvironmentManager>,
    fs_watch_manager: FsWatchManager,
    handles: Arc<Mutex<HashMap<ConnectionId, FileHandleManager>>>,
}

impl FsRequestProcessor {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        fs_watch_manager: FsWatchManager,
    ) -> Self {
        Self {
            environment_manager,
            fs_watch_manager,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn file_system(&self) -> Result<Arc<dyn ExecutorFileSystem>, JSONRPCErrorError> {
        self.environment_manager
            .try_local_environment()
            .map(|environment| environment.get_filesystem())
            .ok_or_else(|| internal_error("local filesystem is not configured"))
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        let handles = self.handles.lock().await.remove(&connection_id);
        if let Some(handles) = handles {
            handles.close_all().await;
        }
        self.fs_watch_manager.connection_closed(connection_id).await;
    }

    async fn handles(&self, connection_id: ConnectionId) -> FileHandleManager {
        self.handles
            .lock()
            .await
            .entry(connection_id)
            .or_default()
            .clone()
    }

    async fn existing_handles(&self, connection_id: ConnectionId) -> Option<FileHandleManager> {
        self.handles.lock().await.get(&connection_id).cloned()
    }

    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let bytes = self
            .file_system()?
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
        let bytes = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "fs/writeFile requires valid base64 dataBase64: {err}"
            ))
        })?;
        self.file_system()?
            .write_file(&params.path, bytes, /*sandbox*/ None)
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn read_file_open(
        &self,
        connection_id: ConnectionId,
        params: FsReadFileOpenParams,
    ) -> Result<FsReadFileOpenResponse, JSONRPCErrorError> {
        let max_chunk_bytes = self
            .handles(connection_id)
            .await
            .open_read(
                self.file_system()?,
                params.handle_id,
                &params.path,
                /*sandbox*/ None,
            )
            .await
            .map_err(map_file_handle_error)?;
        Ok(FsReadFileOpenResponse {
            max_chunk_bytes: u32::try_from(max_chunk_bytes)
                .map_err(|_| internal_error("file read chunk limit exceeds u32"))?,
        })
    }

    pub(crate) async fn read_file_read(
        &self,
        connection_id: ConnectionId,
        params: FsReadFileReadParams,
    ) -> Result<FsReadFileReadResponse, JSONRPCErrorError> {
        let chunk = self
            .handles(connection_id)
            .await
            .read(
                &params.handle_id,
                params.offset,
                params.max_bytes.map(|value| value as usize),
            )
            .await
            .map_err(map_file_handle_error)?;
        Ok(FsReadFileReadResponse {
            data_base64: STANDARD.encode(chunk.data),
            eof: chunk.eof,
        })
    }

    pub(crate) async fn read_file_stat(
        &self,
        connection_id: ConnectionId,
        params: FsReadFileStatParams,
    ) -> Result<FsReadFileStatResponse, JSONRPCErrorError> {
        let metadata = self
            .handles(connection_id)
            .await
            .stat_read(&params.handle_id)
            .await
            .map_err(map_file_handle_error)?;
        Ok(FsReadFileStatResponse {
            size_bytes: metadata.size_bytes,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_file_close(
        &self,
        connection_id: ConnectionId,
        params: FsReadFileCloseParams,
    ) -> Result<FsReadFileCloseResponse, JSONRPCErrorError> {
        if let Some(handles) = self.existing_handles(connection_id).await {
            handles.close(&params.handle_id).await;
        }
        Ok(FsReadFileCloseResponse {})
    }

    pub(crate) async fn write_file_open(
        &self,
        connection_id: ConnectionId,
        params: FsWriteFileOpenParams,
    ) -> Result<FsWriteFileOpenResponse, JSONRPCErrorError> {
        let max_chunk_bytes = self
            .handles(connection_id)
            .await
            .open_write(
                self.file_system()?,
                params.handle_id,
                &params.path,
                /*sandbox*/ None,
            )
            .await
            .map_err(map_file_handle_error)?;
        Ok(FsWriteFileOpenResponse {
            max_chunk_bytes: u32::try_from(max_chunk_bytes)
                .map_err(|_| internal_error("file write chunk limit exceeds u32"))?,
        })
    }

    pub(crate) async fn write_file_write(
        &self,
        connection_id: ConnectionId,
        params: FsWriteFileWriteParams,
    ) -> Result<FsWriteFileWriteResponse, JSONRPCErrorError> {
        let data = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "fs/writeFile/write requires valid base64 dataBase64: {err}"
            ))
        })?;
        self.handles(connection_id)
            .await
            .write(&params.handle_id, &data)
            .await
            .map_err(map_file_handle_error)?;
        Ok(FsWriteFileWriteResponse {})
    }

    pub(crate) async fn write_file_close(
        &self,
        connection_id: ConnectionId,
        params: FsWriteFileCloseParams,
    ) -> Result<FsWriteFileCloseResponse, JSONRPCErrorError> {
        if let Some(handles) = self.existing_handles(connection_id).await {
            handles.close(&params.handle_id).await;
        }
        Ok(FsWriteFileCloseResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        self.file_system()?
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
        let metadata = self
            .file_system()?
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
        let entries = self
            .file_system()?
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
        self.file_system()?
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
        self.file_system()?
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

    pub(crate) async fn watch(
        &self,
        connection_id: ConnectionId,
        params: FsWatchParams,
    ) -> Result<FsWatchResponse, JSONRPCErrorError> {
        self.file_system()?;
        self.fs_watch_manager.watch(connection_id, params).await
    }

    pub(crate) async fn unwatch(
        &self,
        connection_id: ConnectionId,
        params: FsUnwatchParams,
    ) -> Result<FsUnwatchResponse, JSONRPCErrorError> {
        self.file_system()?;
        self.fs_watch_manager.unwatch(connection_id, params).await
    }
}

fn map_fs_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::InvalidInput {
        invalid_request(err.to_string())
    } else {
        internal_error(err.to_string())
    }
}

fn map_file_handle_error(err: io::Error) -> JSONRPCErrorError {
    match err.kind() {
        io::ErrorKind::AlreadyExists | io::ErrorKind::InvalidInput | io::ErrorKind::NotFound => {
            invalid_request(err.to_string())
        }
        _ => internal_error(err.to_string()),
    }
}
