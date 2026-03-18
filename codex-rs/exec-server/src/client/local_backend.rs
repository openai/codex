use std::sync::Arc;

use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::server::ExecServerHandler;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCopyResponse;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsGetMetadataResponse;
use codex_app_server_protocol::FsReadDirectoryParams;
use codex_app_server_protocol::FsReadDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;

use super::ExecServerError;

#[derive(Clone)]
pub(super) struct LocalBackend {
    handler: Arc<ExecServerHandler>,
}

impl LocalBackend {
    pub(super) fn new(handler: ExecServerHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub(super) async fn shutdown(&self) {
        self.handler.shutdown().await;
    }

    pub(super) async fn initialize(&self) -> Result<InitializeResponse, ExecServerError> {
        self.handler
            .initialize()
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn initialized(&self) -> Result<(), ExecServerError> {
        self.handler
            .initialized()
            .map_err(ExecServerError::Protocol)
    }

    pub(super) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, ExecServerError> {
        self.handler
            .exec(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, ExecServerError> {
        self.handler
            .exec_read(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn exec_write(
        &self,
        params: WriteParams,
    ) -> Result<WriteResponse, ExecServerError> {
        self.handler
            .exec_write(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn terminate(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, ExecServerError> {
        self.handler
            .terminate(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, ExecServerError> {
        self.handler
            .fs_read_file(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, ExecServerError> {
        self.handler
            .fs_write_file(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, ExecServerError> {
        self.handler
            .fs_create_directory(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, ExecServerError> {
        self.handler
            .fs_get_metadata(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, ExecServerError> {
        self.handler
            .fs_read_directory(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, ExecServerError> {
        self.handler
            .fs_remove(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn fs_copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, ExecServerError> {
        self.handler
            .fs_copy(params)
            .await
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }
}
