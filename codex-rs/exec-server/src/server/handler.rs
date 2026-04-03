use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

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
use codex_app_server_protocol::JSONRPCErrorError;

use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::rpc::invalid_request;
use crate::server::file_system_handler::FileSystemHandler;
use crate::server::session_registry::SessionHandle;

const MAX_LONG_POLL_SLICE: Duration = Duration::from_millis(50);

pub(crate) struct ExecServerHandler {
    session: SessionHandle,
    file_system: FileSystemHandler,
    initialize_requested: AtomicBool,
    initialized: AtomicBool,
}

impl ExecServerHandler {
    pub(crate) fn new(session: SessionHandle) -> Self {
        Self {
            session,
            file_system: FileSystemHandler::default(),
            initialize_requested: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.session.detach().await;
    }

    pub(crate) fn is_current_attachment(&self) -> bool {
        self.session.is_current_attachment()
    }

    pub(crate) fn initialize(
        &self,
        _params: InitializeParams,
    ) -> Result<InitializeResponse, JSONRPCErrorError> {
        if self.initialize_requested.swap(true, Ordering::SeqCst) {
            return Err(invalid_request(
                "initialize may only be sent once per connection".to_string(),
            ));
        }

        Ok(InitializeResponse {
            session_id: self.session.session_id().to_string(),
        })
    }

    pub(crate) fn initialized(&self) -> Result<(), String> {
        self.require_current_attachment_string()?;
        if !self.initialize_requested.load(Ordering::SeqCst) {
            return Err("received `initialized` notification before `initialize`".into());
        }
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        self.session.process().exec(params).await
    }

    pub(crate) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;

        let total_wait = Duration::from_millis(params.wait_ms.unwrap_or(0));
        if total_wait.is_zero() {
            return self.session.process().exec_read(params).await;
        }

        let deadline = Instant::now() + total_wait;
        loop {
            let now = Instant::now();
            let wait_ms = deadline
                .saturating_duration_since(now)
                .min(MAX_LONG_POLL_SLICE)
                .as_millis() as u64;
            let response = self
                .session
                .process()
                .exec_read(ReadParams {
                    wait_ms: Some(wait_ms),
                    ..params.clone()
                })
                .await?;
            self.require_current_attachment()?;

            if !response.chunks.is_empty()
                || response.exited
                || response.closed
                || wait_ms == 0
                || Instant::now() >= deadline
            {
                return Ok(response);
            }
        }
    }

    pub(crate) async fn exec_write(
        &self,
        params: WriteParams,
    ) -> Result<WriteResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        self.session.process().exec_write(params).await
    }

    pub(crate) async fn terminate(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        self.session.process().terminate(params).await
    }

    pub(crate) async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.read_file(params).await
    }

    pub(crate) async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.write_file(params).await
    }

    pub(crate) async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.create_directory(params).await
    }

    pub(crate) async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.get_metadata(params).await
    }

    pub(crate) async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.read_directory(params).await
    }

    pub(crate) async fn fs_remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.remove(params).await
    }

    pub(crate) async fn fs_copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        self.require_initialized_for("filesystem")?;
        self.file_system.copy(params).await
    }

    fn require_initialized_for(&self, method_family: &str) -> Result<(), JSONRPCErrorError> {
        self.require_current_attachment()?;
        if !self.initialize_requested.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must call initialize before using {method_family} methods"
            )));
        }
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must send initialized before using {method_family} methods"
            )));
        }
        Ok(())
    }

    fn require_current_attachment(&self) -> Result<(), JSONRPCErrorError> {
        if self.is_current_attachment() {
            Ok(())
        } else {
            Err(invalid_request(
                "session has been resumed by another connection".to_string(),
            ))
        }
    }

    fn require_current_attachment_string(&self) -> Result<(), String> {
        if self.is_current_attachment() {
            Ok(())
        } else {
            Err("session has been resumed by another connection".to_string())
        }
    }
}

#[cfg(test)]
mod tests;
