use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCErrorError;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::ExecServerRuntimePaths;
use crate::http_request::PendingHttpBodyStream;
use crate::http_request::run_http_request;
use crate::http_request::stream_http_body;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileParams;
use crate::protocol::FsWriteFileResponse;
use crate::protocol::HttpRequestParams;
use crate::protocol::HttpRequestResponse;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::invalid_params;
use crate::rpc::invalid_request;
use crate::server::file_system_handler::FileSystemHandler;
use crate::server::session_registry::SessionHandle;
use crate::server::session_registry::SessionRegistry;

#[derive(Default)]
struct HttpBodyStreamRegistry {
    reserved_ids: HashMap<String, Option<JoinHandle<()>>>,
}

pub(crate) struct ExecServerHandler {
    session_registry: Arc<SessionRegistry>,
    notifications: RpcNotificationSender,
    session: StdMutex<Option<SessionHandle>>,
    http_body_streams: Mutex<HttpBodyStreamRegistry>,
    file_system: FileSystemHandler,
    initialize_requested: AtomicBool,
    initialized: AtomicBool,
}

impl ExecServerHandler {
    pub(crate) fn new(
        session_registry: Arc<SessionRegistry>,
        notifications: RpcNotificationSender,
        runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        Self {
            session_registry,
            notifications,
            session: StdMutex::new(None),
            http_body_streams: Mutex::new(HttpBodyStreamRegistry::default()),
            file_system: FileSystemHandler::new(runtime_paths),
            initialize_requested: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
        }
    }

    pub(crate) async fn shutdown(&self) {
        let tasks = {
            let mut http_body_streams = self.http_body_streams.lock().await;
            let tasks = http_body_streams
                .reserved_ids
                .drain()
                .filter_map(|(_, task)| task)
                .collect::<Vec<_>>();
            tasks
        };
        for task in tasks {
            task.abort();
        }
        if let Some(session) = self.session() {
            session.detach().await;
        }
    }

    pub(crate) fn is_session_attached(&self) -> bool {
        self.session()
            .is_none_or(|session| session.is_session_attached())
    }

    pub(crate) async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResponse, JSONRPCErrorError> {
        if self.initialize_requested.swap(true, Ordering::SeqCst) {
            return Err(invalid_request(
                "initialize may only be sent once per connection".to_string(),
            ));
        }

        let session = match self
            .session_registry
            .attach(params.resume_session_id.clone(), self.notifications.clone())
            .await
        {
            Ok(session) => session,
            Err(error) => {
                self.initialize_requested.store(false, Ordering::SeqCst);
                return Err(error);
            }
        };
        let session_id = session.session_id().to_string();
        tracing::debug!(
            session_id,
            connection_id = %session.connection_id(),
            "exec-server session attached"
        );
        *self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(session);
        Ok(InitializeResponse { session_id })
    }

    pub(crate) fn initialized(&self) -> Result<(), String> {
        if !self.initialize_requested.load(Ordering::SeqCst) {
            return Err("received `initialized` notification before `initialize`".into());
        }
        self.require_session_attached()
            .map_err(|error| error.message)?;
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, JSONRPCErrorError> {
        let session = self.require_initialized_for("exec")?;
        session.process().exec(params).await
    }

    pub(crate) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, JSONRPCErrorError> {
        let session = self.require_initialized_for("exec")?;
        let response = session.process().exec_read(params).await?;
        self.require_session_attached()?;
        Ok(response)
    }

    pub(crate) async fn exec_write(
        &self,
        params: WriteParams,
    ) -> Result<WriteResponse, JSONRPCErrorError> {
        let session = self.require_initialized_for("exec")?;
        session.process().exec_write(params).await
    }

    pub(crate) async fn terminate(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, JSONRPCErrorError> {
        let session = self.require_initialized_for("exec")?;
        session.process().terminate(params).await
    }

    pub(crate) async fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> Result<(HttpRequestResponse, Option<PendingHttpBodyStream>), JSONRPCErrorError> {
        self.require_initialized_for("http")?;
        let request_id = if params.stream_response {
            Some(params.request_id.clone().ok_or_else(|| {
                invalid_params("http/request streamResponse requires requestId".to_string())
            })?)
        } else {
            None
        };
        if let Some(request_id) = request_id.as_deref() {
            self.reserve_http_body_stream(request_id).await?;
        }
        match run_http_request(params).await {
            Ok((response, pending_stream)) => Ok((response, pending_stream)),
            Err(error) => {
                if let Some(request_id) = request_id.as_deref() {
                    self.release_http_body_stream(request_id).await;
                }
                Err(error)
            }
        }
    }

    pub(crate) async fn start_http_body_stream(
        self: &Arc<Self>,
        pending_stream: PendingHttpBodyStream,
    ) {
        let request_id = pending_stream.request_id.clone();
        let finished_request_id = request_id.clone();
        let handler = Arc::clone(self);
        let notifications = self.notifications.clone();
        let task = tokio::spawn(async move {
            stream_http_body(pending_stream, notifications).await;
            handler.release_http_body_stream(&finished_request_id).await;
        });
        let mut http_body_streams = self.http_body_streams.lock().await;
        if let Some(entry) = http_body_streams.reserved_ids.get_mut(&request_id) {
            *entry = Some(task);
        } else {
            task.abort();
        }
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

    fn require_initialized_for(
        &self,
        method_family: &str,
    ) -> Result<SessionHandle, JSONRPCErrorError> {
        if !self.initialize_requested.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must call initialize before using {method_family} methods"
            )));
        }
        let session = self.require_session_attached()?;
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must send initialized before using {method_family} methods"
            )));
        }
        Ok(session)
    }

    fn require_session_attached(&self) -> Result<SessionHandle, JSONRPCErrorError> {
        let Some(session) = self.session() else {
            return Err(invalid_request(
                "client must call initialize before using methods".to_string(),
            ));
        };
        if session.is_session_attached() {
            return Ok(session);
        }

        Err(invalid_request(
            "session has been resumed by another connection".to_string(),
        ))
    }

    fn session(&self) -> Option<SessionHandle> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    async fn reserve_http_body_stream(&self, request_id: &str) -> Result<(), JSONRPCErrorError> {
        let mut http_body_streams = self.http_body_streams.lock().await;
        if http_body_streams.reserved_ids.contains_key(request_id) {
            return Err(invalid_params(format!(
                "http/request streamResponse requestId `{request_id}` is already active"
            )));
        }
        http_body_streams
            .reserved_ids
            .insert(request_id.to_string(), None);
        Ok(())
    }

    pub(crate) async fn release_http_body_stream(&self, request_id: &str) {
        let mut http_body_streams = self.http_body_streams.lock().await;
        http_body_streams.reserved_ids.remove(request_id);
    }
}

#[cfg(test)]
mod tests;
